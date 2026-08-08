public class GuardWithoutExpression {
  String f(Object o) { return switch (o) { case Integer i when -> "x"; default -> "y"; }; }
}
