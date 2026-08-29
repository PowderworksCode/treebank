// Records (17) and record patterns (21). The record header is the shape
// worth guarding: `record Point(int x, int y)` puts a parameter list where
// a class body would start, and the compact constructor then puts a body
// where a parameter list would be.
package fixtures;

import java.util.List;
import java.util.Objects;

record Records(int x, int y) {

    // A compact constructor: no parameter list at all.
    Records {
        if (x < 0) {
            throw new IllegalArgumentException("x");
        }
    }

    // A canonical constructor written out long-hand, and a secondary one
    // that delegates.
    Records(int x, int y, boolean unused) {
        this(x, y);
    }

    // Static and instance members, a static factory, and an explicit
    // accessor overriding the implicit one.
    static final Records ORIGIN = new Records(0, 0);

    static Records of(int x) {
        return new Records(x, x);
    }

    @Override
    public int x() {
        return x;
    }

    int sum() {
        return x() + y();
    }
}

// An empty header, a generic record, a record with a varargs component,
// annotated components, and one implementing an interface.
record Empty() {
}

record Pair<A, B>(A first, B second) {
    <C> Pair<A, C> withSecond(C value) {
        return new Pair<>(first, value);
    }
}

record Varargs(String name, int... values) {
}

record Annotated(@Deprecated String name, java.util.@SuppressWarnings("x") List<String> items) {
}

interface Shape {
}

record Circle(double radius) implements Shape {
}

record Rectangle(double width, double height) implements Shape {
}

class RecordUse {

    // A local record -- a record declaration in statement position.
    int local() {
        record Local(int value) {
        }
        return new Local(1).value();
    }

    // A nested record inside a record, and a record inside an interface.
    record Outer(int a) {
        record Inner(int b) {
        }
    }

    // Record patterns in `instanceof`, including a nested one and one
    // whose components are `var`.
    boolean instanceOfPattern(Object o) {
        if (o instanceof Circle(double r)) {
            return r > 0;
        }
        if (o instanceof Rectangle(double w, double h) && w > h) {
            return true;
        }
        if (o instanceof Pair(Circle(var r), var second)) {
            return r > 0 && second != null;
        }
        return false;
    }

    // Record patterns in a switch, with and without a guard.
    String switchPattern(Object o) {
        return switch (o) {
            case Circle(double r) when r > 10 -> "big circle";
            case Circle(double r) -> "circle " + r;
            case Rectangle(double w, double h) -> "rect " + w + "x" + h;
            case Pair(Circle c, Object second) -> "pair " + Objects.toString(second);
            case List<?> list -> "list " + list.size();
            default -> "other";
        };
    }
}
