// Format through TypeScript's own language service rather than a third-party
// style tool. stdin is one path per line; stdout is one JSON record per path.
import fs from "node:fs";
import ts from "typescript";

const settings = {
  indentSize: 2,
  tabSize: 2,
  convertTabsToSpaces: true,
  newLineCharacter: "\n",
  insertSpaceAfterCommaDelimiter: true,
  insertSpaceAfterSemicolonInForStatements: true,
  insertSpaceBeforeAndAfterBinaryOperators: true,
  insertSpaceAfterKeywordsInControlFlowStatements: true,
  insertSpaceAfterFunctionKeywordForAnonymousFunctions: true,
  insertSpaceBeforeFunctionParenthesis: false,
  placeOpenBraceOnNewLineForFunctions: false,
  placeOpenBraceOnNewLineForControlBlocks: false,
};

function applyEdits(source, edits) {
  // Changes use offsets in the original source. Applying from the end keeps
  // every earlier offset valid regardless of replacement length.
  const ordered = [...edits].sort((a, b) => b.span.start - a.span.start);
  for (const edit of ordered) {
    source = source.slice(0, edit.span.start) + edit.newText
      + source.slice(edit.span.start + edit.span.length);
  }
  return source;
}

for (const raw of fs.readFileSync(0, "utf8").split("\n")) {
  const path = raw.trim();
  if (!path) continue;
  try {
    const source = fs.readFileSync(path, "utf8");
    const kind = path.endsWith(".jsx")
      ? ts.ScriptKind.JSX
      : /\.(js|mjs|cjs)$/.test(path)
        ? ts.ScriptKind.JS
        : path.endsWith(".tsx")
          ? ts.ScriptKind.TSX
          : ts.ScriptKind.TS;
    const file = ts.createSourceFile(path, source, ts.ScriptTarget.Latest, true, kind);
    const host = {
      getNewLine: () => "\n",
      getCanonicalFileName: (name) => name,
      useCaseSensitiveFileNames: () => true,
    };
    const context = ts.formatting.getFormatContext(settings, host);
    const edits = ts.formatting.formatDocument(file, context);
    process.stdout.write(`${JSON.stringify({ path, source: applyEdits(source, edits) })}\n`);
  } catch (error) {
    process.stdout.write(`${JSON.stringify({ path, skipped: String(error.message ?? error) })}\n`);
  }
}
