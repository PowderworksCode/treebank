// Switch, in all four forms the language has grown: the colon statement
// (1), the arrow statement and the switch expression (14), and pattern
// labels with guards (21). The colon and arrow forms share a case label
// but disagree about who owns the separator, which is why
// `CASE <- switch_label_group` and `CASE <- switch_rule` are declared
// granularity differences in shape_policy.toml.
package fixtures;

import java.util.List;

class SwitchForms {

    enum Kind {
        FIRST,
        SECOND,
        THIRD
    }

    sealed interface Node permits Leaf, Branch {
    }

    record Leaf(int value) implements Node {
    }

    record Branch(Node left, Node right) implements Node {
    }

    // The colon statement form: fall-through, grouped labels, a default in
    // the middle, and a block-bodied arm.
    int colonStatement(Kind kind) {
        int result = 0;
        switch (kind) {
            case FIRST:
                result = 1;
                break;
            case SECOND:
            case THIRD: {
                result = 2;
                break;
            }
            default:
                result = -1;
        }
        return result;
    }

    // A colon switch that falls off the end with no default, and one over
    // a String and over an int -- the three selector types that existed
    // before patterns.
    void colonSelectors(String s, int i, char c) {
        switch (s) {
            case "a":
                break;
            default:
                break;
        }
        switch (i) {
            case 1:
            case 2:
                break;
            default:
                break;
        }
        switch (c) {
            case 'x':
                break;
            default:
                break;
        }
    }

    // The arrow statement form: no fall-through, multi-label arms, a
    // block body, and a `default`.
    void arrowStatement(Kind kind) {
        switch (kind) {
            case FIRST -> System.out.println("first");
            case SECOND, THIRD -> {
                System.out.println("rest");
            }
            default -> System.out.println("none");
        }
    }

    // The switch EXPRESSION with arrows, in the two body shapes: an
    // expression arm and a block arm that `yield`s.
    int arrowExpression(Kind kind) {
        return switch (kind) {
            case FIRST -> 1;
            case SECOND, THIRD -> {
                int doubled = 2 * 2;
                yield doubled;
            }
        };
    }

    // A switch expression with COLON labels, which still needs `yield`
    // rather than `break` -- the form people forget exists.
    int colonExpression(Kind kind) {
        return switch (kind) {
            case FIRST:
                yield 1;
            case SECOND:
            case THIRD:
                yield 2;
            default:
                yield -1;
        };
    }

    // Qualified enum constants as labels, and a switch expression in
    // argument, ternary and assignment position.
    int positions(Kind kind, boolean flag) {
        int assigned = switch (kind) {
            case Kind.FIRST -> 1;
            case Kind.SECOND -> 2;
            case Kind.THIRD -> 3;
        };
        int ternary = flag ? switch (kind) {
            case FIRST -> 1;
            default -> 0;
        } : 0;
        return Math.max(assigned, ternary);
    }

    // Type patterns, a `null` label, `null, default` together, and
    // `when` guards -- the Java 21 label vocabulary.
    String patterns(Object o) {
        return switch (o) {
            case null -> "null";
            case Integer i when i > 10 -> "big int " + i;
            case Integer i -> "int " + i;
            case String s when s.isEmpty() -> "empty string";
            case String s -> "string " + s;
            case int[] array -> "int array " + array.length;
            case List<?> list -> "list " + list.size();
            default -> "other";
        };
    }

    // Record patterns as labels, nested, with `var` components and a
    // guard -- exhaustive over a sealed hierarchy, so no default.
    int sealedExhaustive(Node node) {
        return switch (node) {
            case Leaf(int value) when value < 0 -> 0;
            case Leaf(int value) -> value;
            case Branch(Leaf(var left), Leaf(var right)) -> left + right;
            case Branch(Node left, Node right) -> sealedExhaustive(left) + sealedExhaustive(right);
        };
    }

    // `null, default` as a combined label, which is the only place
    // `default` may share an arm with a constant.
    String nullDefault(Object o) {
        return switch (o) {
            case String s -> s;
            case null, default -> "fallback";
        };
    }

    // A labelled switch containing `break label`, and a nested switch --
    // where `break` means two different things one line apart.
    int labelledBreak(Kind kind) {
        int result = 0;
        outer: switch (kind) {
            case FIRST:
                switch (kind) {
                    case FIRST:
                        result = 1;
                        break;
                    default:
                        break outer;
                }
                break;
            default:
                result = -1;
        }
        return result;
    }
}
