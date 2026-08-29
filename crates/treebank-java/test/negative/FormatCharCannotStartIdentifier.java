// U+200D ZERO WIDTH JOINER is `Cf`. isJavaIdentifierPart accepts it,
// isJavaIdentifierStart does not.
class FormatCharCannotStartIdentifier {
    int ‍zwj = 1;
}
