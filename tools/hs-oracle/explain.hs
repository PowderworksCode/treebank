-- | Diagnostic sibling of check.hs: same stdin contract, but emits the first
-- parse error's GHC diagnostic code, span and message instead of a bare
-- verdict, so failures cluster by cause.
--
--   stdin:  @\<path\>@, or @\<path\>\\t-XExt\\t-XExt…@, one per line
--   stdout: @\<path\>\\tvalid@, or @\<path\>\\tGHC-NNNNN\\t\<span\>\\t\<message\>@
--
-- The diagnostic CODE rather than the rendered message is the handle that
-- clusters: GHC-51179 is "illegal \case" whatever the file, whatever the
-- span, and whatever wording the release settles on.
{-# LANGUAGE CPP #-}
{-# LANGUAGE LambdaCase #-}
{-# LANGUAGE ScopedTypeVariables #-}
{-# LANGUAGE TypeApplications #-}
module Main (main) where

import qualified GHC
import qualified GHC.Parser as Parser
import GHC.Data.FastString (mkFastString)
import GHC.Data.StringBuffer (StringBuffer, hGetStringBuffer)
import GHC.Driver.Config.Parser (initParserOpts)
import GHC.Driver.Session (DynFlags, parseDynamicFilePragma)
import GHC.Parser.Errors.Types (PsMessage)
import GHC.Parser.Header (getOptions)
import GHC.Parser.Lexer (ParseResult (..), PState, getPsErrorMessages, initParserState, unP)
import GHC.Types.Error
  ( defaultDiagnosticOpts, diagnosticCode, diagnosticMessage, errMsgDiagnostic
  , errMsgSpan, getMessages, unDecorated )
import GHC.Types.SourceError (SourceError)
import GHC.Types.SrcLoc (mkRealSrcLoc)
import GHC.Utils.Outputable (ppr, showSDocUnsafe)

import Control.Exception (evaluate, try)
import Control.Monad.IO.Class (MonadIO, liftIO)
import Data.Foldable (toList)
import Data.List (isPrefixOf)
import System.Environment (lookupEnv)
import System.IO
import System.Directory (doesDirectoryExist)
import System.Process (readProcess)

main :: IO ()
main = do
  hSetBuffering stdout LineBuffering
  libdir <- resolveLibdir
  input <- getContents
  GHC.runGhc (Just libdir) $ do
    base <- GHC.getSessionDynFlags
    mapM_ (\l -> explain base l >>= liftIO . putStrLn) (lines input)

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
    fromPath = unwords . words <$> readProcess "ghc" ["--print-libdir"] ""

explain :: DynFlags -> String -> GHC.Ghc String
explain base line = do
  let (path, rest) = break (== '\t') line
      flags = filter (not . null) (splitOn '\t' (drop 1 rest))
  buf <- liftIO (hGetStringBuffer path)
  r <- liftIO . try $ do
    packaged <- applyX base flags
    let (_, filePragmas) = getOptions (initParserOpts packaged) buf path
    evaluate =<< applyX packaged (map GHC.unLoc filePragmas)
  pure . (path ++) $ case r of
    Left (_ :: SourceError) -> "\tGHC-68686\t" ++ path ++ "\tmalformed LANGUAGE/OPTIONS_GHC pragma"
    Right dflags -> case parseWith dflags path buf of
      Nothing -> "\tvalid"
      Just pst -> case firstError pst of
        Nothing -> "\tGHC-00000\t" ++ path ++ "\tunrecoverable parse failure, no message"
        Just m -> "\t" ++ m

parseWith :: DynFlags -> FilePath -> StringBuffer -> Maybe PState
parseWith dflags path buf =
  let loc = mkRealSrcLoc (mkFastString path) 1 1
      st = initParserState (initParserOpts dflags) buf loc
  in case unP Parser.parseModule st of
       PFailed pst -> Just pst
       POk pst _ | null (toList (getMessages (getPsErrorMessages pst))) -> Nothing
                 | otherwise -> Just pst

firstError :: PState -> Maybe String
firstError pst = case toList (getMessages (getPsErrorMessages pst)) of
  [] -> Nothing
  (e : _) -> Just $
    maybe "GHC-?????" (showSDocUnsafe . ppr) (diagnosticCode (errMsgDiagnostic e))
    ++ "\t" ++ showSDocUnsafe (ppr (errMsgSpan e))
    ++ "\t" ++ unwords (concatMap (words . showSDocUnsafe)
                          (unDecorated (diagnosticMessage
                             (defaultDiagnosticOpts @PsMessage) (errMsgDiagnostic e))))

applyX :: MonadIO m => DynFlags -> [String] -> m DynFlags
applyX dflags args
  | null xs = pure dflags
  | otherwise = (\(d, _, _) -> d) <$> parseDynamicFilePragma dflags (map GHC.noLoc xs)
  where xs = filter ("-X" `isPrefixOf`) args

splitOn :: Char -> String -> [String]
splitOn c s = case break (== c) s of
  (a, []) -> [a]
  (a, _ : b) -> a : splitOn c b
