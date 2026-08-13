-- | Syntax-only Haskell validity check for the treebank oracle.
--
--   stdin:  @\<path\>@, or @\<path\>\\t-XExt\\t-XExt…@, one per line
--   stdout: @\<path\>\\tvalid|invalid@, one per line
--
-- The reference parser is GHC's own: 'GHC.Parser.parseModule', the exact
-- entry point the compiler uses to turn a file's text into a syntax tree.
-- It resolves no import, loads no interface file, runs no Template Haskell
-- and does not typecheck, so a file is judged entirely on its own text —
-- the property that makes CPython's @compile()@ and @ts.createSourceFile@
-- usable the same way.
--
-- WHICH PARSER. The roadmap names @ghc-lib-parser@. This uses the @ghc@
-- library that ships inside the GHC installation instead, which is the same
-- parser source: @ghc-lib-parser@ is GHC's parser repackaged for tools that
-- must not depend on the compiler they are built by. Measured here, the
-- difference is entirely cost: this file compiles in 9 s against the
-- bundled library, where @ghc-lib-parser-9.14.1.20251220@ took 8m02s to
-- build (alex and happy first) for a 324 MB store artifact — and it still
-- needs the 3.2 GB GHC to be built against, so it removes no footprint.
-- What it would buy is a parser version pinnable independently of the
-- toolchain; ledger.json pins the toolchain instead, and 'versionCheck'
-- below refuses to run under any other one.
--
-- WHICH GHC. Haskell has the property zig has: the answer to "is this file
-- valid?" is per compiler version, because the language grows extensions.
-- So the version is half the oracle's output, and it is recorded in
-- ledger.json's @oracle@ field beside the verdict counts. Unlike zig, the
-- version is fixed at COMPILE time here (the parser is a library, not a
-- subprocess), so the check that matters is that the libdir found at run
-- time belongs to the same GHC this was built against.
{-# LANGUAGE CPP #-}
{-# LANGUAGE LambdaCase #-}
{-# LANGUAGE ScopedTypeVariables #-}
module Main (main) where

import qualified GHC
import qualified GHC.Parser as Parser
import GHC.Data.FastString (mkFastString)
import GHC.Data.StringBuffer (StringBuffer, hGetStringBuffer)
import GHC.Driver.Config.Parser (initParserOpts)
import GHC.Driver.Session (DynFlags, parseDynamicFilePragma)
import GHC.Parser.Header (getOptions)
import GHC.Parser.Lexer (ParseResult (..), getPsErrorMessages, initParserState, unP)
import GHC.Settings.Config (cProjectVersion)
import GHC.Types.Error (isEmptyMessages)
import GHC.Types.SourceError (SourceError)
import GHC.Types.SrcLoc (mkRealSrcLoc)

import Control.Exception (evaluate, try)
import Control.Monad.IO.Class (MonadIO, liftIO)
import Data.List (isPrefixOf)
import System.Environment (lookupEnv)
import System.Exit (exitFailure)
import System.IO
import System.Directory (doesDirectoryExist)
import System.Process (readProcess)

main :: IO ()
main = do
  hSetBuffering stdout LineBuffering
  libdir <- resolveLibdir
  versionCheck libdir
  input <- getContents
  GHC.runGhc (Just libdir) $ do
    base <- GHC.getSessionDynFlags
    mapM_ (\l -> answer base l >>= liftIO . putStrLn) (lines input)

-- | Where GHC's own package database lives.
--
-- Baked in at build time (build.sh passes the building compiler's
-- @--print-libdir@) rather than looked up by running @ghc@, because a sweep
-- runs this binary from wherever the driver happens to be and the first one
-- to do so found no @ghc@ on PATH and took the whole sweep down. A tool
-- under tools/ is expected to work once built, the way c-oracle and
-- go-oracle do.
--
-- Precedence: the environment wins, so a second GHC can be pointed at
-- deliberately; then the compiler that built this; then PATH, which is the
-- only path left if the build tree was moved or copied to another machine.
resolveLibdir :: IO FilePath
resolveLibdir = lookupEnv "TREEBANK_GHC_LIBDIR" >>= \case
  Just d -> pure d
  Nothing -> do
#ifdef TREEBANK_GHC_LIBDIR
    let built = TREEBANK_GHC_LIBDIR
    there <- doesDirectoryExist built
    if there then pure built else fromPath
#else
    fromPath
#endif
  where
    fromPath = trim <$> readProcess "ghc" ["--print-libdir"] ""

-- | The library this is linked against and the libdir found at run time must
-- come from the same GHC, or the verdicts belong to a version nobody
-- recorded. Same reasoning as check.lua refusing to run under a @_VERSION@
-- other than the pinned one: an oracle that quietly answers for an
-- unrecorded dialect is worse than one that does not answer.
versionCheck :: FilePath -> IO ()
versionCheck libdir = do
  found <- (Just . trim <$> readProcess "ghc" ["--numeric-version"] "")
             `orElse` pure Nothing
  case found of
    Just v | v /= cProjectVersion -> do
      hPutStrLn stderr $
        "hs-oracle: built against GHC " ++ cProjectVersion ++ " but `ghc` on PATH is "
        ++ v ++ " (libdir " ++ libdir ++ "). Rebuild with tools/hs-oracle/build.sh, or"
        ++ " point TREEBANK_GHC_LIBDIR at the matching install."
      exitFailure
    _ -> pure ()
  where orElse a b = try a >>= \case
          Right x -> pure x
          Left (_ :: IOError) -> b

-- | One request line to one verdict line. The tab-separated tail is the
-- per-file flag list, which is how a language whose parser is configured
-- OUTSIDE the file gets that configuration in: `Lang::validate` derives
-- @-XExt@ flags from the package's .cabal and sends them alongside the path,
-- the same protocol tools/c-oracle takes its include paths through.
answer :: DynFlags -> String -> GHC.Ghc String
answer base line = do
  let (path, rest) = break (== '\t') line
      flags = filter (not . null) (splitOn '\t' (drop 1 rest))
  v <- verdict base path flags
  pure (path ++ "\t" ++ v)

verdict :: DynFlags -> FilePath -> [String] -> GHC.Ghc String
verdict base path flags = do
  -- Outside the handler below, deliberately: an unreadable file is NOT an
  -- invalid file. A mistyped corpus root has to take the sweep down rather
  -- than turn every grammar failure into noise and report a flawless
  -- grammar, so an I/O error here is fatal and stays fatal.
  buf <- liftIO (hGetStringBuffer path)
  -- A file whose own LANGUAGE/OPTIONS_GHC pragma is malformed makes GHC
  -- THROW out of parseDynamicFilePragma rather than return a verdict, and
  -- one such file used to take the whole batch down with it — 493 files
  -- lost their verdicts mid-run. SourceError is GHC's "this source is bad"
  -- channel and nothing else: panics, installation errors and I/O all
  -- travel as other types and remain fatal. So it is the one exception
  -- that maps to a verdict, and the verdict is `invalid`, which is what
  -- GHC itself does with such a file.
  --
  -- Found on the reject path (corpus files truncated to 60%), which is the
  -- only path validate() ever runs on and the only place a half-written
  -- pragma occurs. The valid path never showed it.
  r <- liftIO . try $ do
    packaged <- applyX base flags
    let (_, filePragmas) = getOptions (initParserOpts packaged) buf path
    evaluate =<< applyX packaged (map GHC.unLoc filePragmas)
  pure $ case r of
    Left (_ :: SourceError) -> "invalid"
    Right dflags -> parseWith dflags path buf

-- | The verdict itself.
--
-- POk is NOT a verdict. GHC's parser accumulates recoverable errors — every
-- @Illegal \\case (use LambdaCase)@ is one — and still returns POk with the
-- tree it managed to build; PFailed is only the unrecoverable path. Reading
-- the constructor alone, 9 of 12 extension-gated constructs in
-- test/battery came back valid that GHC itself rejects. The errors are the
-- verdict.
parseWith :: DynFlags -> FilePath -> StringBuffer -> String
parseWith dflags path buf =
  let loc = mkRealSrcLoc (mkFastString path) 1 1
      st = initParserState (initParserOpts dflags) buf loc
  in case unP Parser.parseModule st of
       PFailed _ -> "invalid"
       POk pst _
         | isEmptyMessages (getPsErrorMessages pst) -> "valid"
         | otherwise -> "invalid"

-- | Only @-X@ flags are honoured. A corpus file's OPTIONS_GHC can name
-- anything the driver accepts — @-F -pgmF hspec-discover@, @-Wall@,
-- @-fplugin@ — and none of that changes what the PARSER accepts, while
-- some of it would make GHC try to run a program from the corpus.
applyX :: MonadIO m => DynFlags -> [String] -> m DynFlags
applyX dflags args
  | null xs = pure dflags
  | otherwise = (\(d, _, _) -> d) <$> parseDynamicFilePragma dflags (map GHC.noLoc xs)
  where xs = filter ("-X" `isPrefixOf`) args

splitOn :: Char -> String -> [String]
splitOn c s = case break (== c) s of
  (a, []) -> [a]
  (a, _ : b) -> a : splitOn c b

trim :: String -> String
trim = unwords . words
