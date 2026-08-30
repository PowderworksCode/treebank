// JLS 3.8: `Character.isJavaIdentifierStart` excludes Mn and Mc, so a
// combining mark may continue a name but never begin one. Widening the
// identifier class for issue #196 must not blur that edge.
class MarkCannotStartIdentifier {
    int ि = 1;
}
