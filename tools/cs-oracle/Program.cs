// Syntax-only C# validity check for the treebank oracle.
//
// stdin:  one file path per line
// stdout: "<path>\tvalid|invalid" per line
//
// The reference parser is Roslyn, the C# compiler's own. CSharpSyntaxTree
// .ParseText runs the parser and nothing else: no binding, no references, no
// project context, so unresolved types are not errors and each file is
// judged on its own — the same property that makes ts.createSourceFile
// usable for TypeScript and JavacTask.parse() for Java. Only diagnostics of
// severity Error count.
//
// The language version is LanguageVersion.Latest, i.e. the newest *stable*
// C# this Roslyn supports, not Preview. A file that needs a preview-only
// feature is not yet valid C#, and recording it as corpus noise is the
// honest answer; calling it valid would report the grammar's rejection of
// unreleased syntax as a grammar gap.

using Microsoft.CodeAnalysis;
using Microsoft.CodeAnalysis.CSharp;
using Microsoft.CodeAnalysis.Text;

using System.Text;

var options = new CSharpParseOptions(LanguageVersion.Latest, DocumentationMode.None, SourceCodeKind.Regular);
var stdout = new StreamWriter(Console.OpenStandardOutput(), new UTF8Encoding(false));

string? line;
while ((line = Console.In.ReadLine()) != null)
{
    var path = line.Trim();
    if (path.Length == 0)
    {
        continue;
    }
    stdout.Write($"{path}\t{(Parses(path, options) ? "valid" : "invalid")}\n");
}
stdout.Flush();

static bool Parses(string path, CSharpParseOptions options)
{
    try
    {
        using var stream = File.OpenRead(path);
        var text = SourceText.From(stream, Encoding.UTF8, canBeEmbedded: false);
        var tree = CSharpSyntaxTree.ParseText(text, options, path);
        foreach (var d in tree.GetDiagnostics())
        {
            if (d.Severity == DiagnosticSeverity.Error)
            {
                return false;
            }
        }
        return true;
    }
    catch (Exception)
    {
        return false;
    }
}
