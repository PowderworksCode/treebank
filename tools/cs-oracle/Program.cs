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

// An unreadable file is NOT an invalid file. Returning false for one looks
// harmless and is not: validate() is only ever called on files the grammar
// already failed, and an invalid verdict records the file as corpus NOISE.
// So a mistyped corpus root would make every path unreadable, every grammar
// failure noise, gap_files zero -- and the sweep would report a flawless
// grammar. A broken oracle must fail loudly, never quietly agree with us;
// the reasoning is spelled out in
// crates/treebank-cli/src/lang/exec_oracle.rs.
//
// So the read is separate from the parse. Roslyn's own diagnostics stay the
// only source of a verdict, and a parser blow-up on the file's content is
// still invalid -- that is the file's fault, not the harness's.
static bool Parses(string path, CSharpParseOptions options)
{
    SourceText text;
    try
    {
        using var stream = File.OpenRead(path);
        text = SourceText.From(stream, Encoding.UTF8, canBeEmbedded: false);
    }
    catch (Exception e) when (e is IOException or UnauthorizedAccessException
                              or ArgumentException or NotSupportedException)
    {
        Console.Error.WriteLine($"cs-oracle: cannot read {path}: {e.Message}");
        Console.Error.WriteLine("cs-oracle: this is an oracle failure, not a verdict; "
            + "check the corpus root");
        Environment.Exit(1);
        throw;
    }

    try
    {
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
