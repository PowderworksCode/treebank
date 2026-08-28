// Generics (5). The angle brackets are the hard part: `<` is also
// less-than, `>>` is also a shift, and `a < b` versus `A<B>` is decided
// arbitrarily far to the right. Every nesting depth below is a place a
// naive lexer closes the wrong number of brackets.
package fixtures;

import java.io.Serializable;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.function.Function;

class Generics<T extends Comparable<T> & Serializable> {

    // Nested type arguments closing two and three brackets at once -- the
    // `>>` and `>>>` shift spellings.
    Map<String, List<String>> two = new HashMap<>();
    Map<String, List<List<String>>> three = new HashMap<>();
    Map<String, Map<String, List<Map<String, String>>>> four = new HashMap<>();

    // Wildcards: unbounded, extends-bounded, super-bounded, and annotated.
    List<?> unbounded = new ArrayList<String>();
    List<? extends Number> covariant = new ArrayList<Integer>();
    List<? super Integer> contravariant = new ArrayList<Number>();
    List<? extends List<? super Integer>> nestedWildcard = new ArrayList<>();

    // The diamond, including on an anonymous class.
    List<String> diamond = new ArrayList<>();
    Comparator<String> anonymousDiamond = new Comparator<>() {
        @Override
        public int compare(String a, String b) {
            return a.compareTo(b);
        }
    };

    // A generic method with a bound, one with multiple bounds, and one
    // whose bound mentions its own parameter.
    <U> U identity(U value) {
        return value;
    }

    <U extends Comparable<? super U>> U max(List<? extends U> values) {
        return values.stream().max(Comparator.naturalOrder()).orElseThrow();
    }

    <U extends Number & Comparable<U>> int compare(U a, U b) {
        return a.compareTo(b);
    }

    // A generic constructor, separate from the class's own parameters.
    <U> Generics(U seed) {
    }

    Generics() {
    }

    // Explicit type arguments at a call site: on `this`, on a qualified
    // receiver, on `super`, and on a constructor invocation.
    void explicitTypeArguments() {
        this.<String>identity("x");
        Generics.<String>staticIdentity("x");
        new <String>Generics("seed");
    }

    static <U> U staticIdentity(U value) {
        return value;
    }

    // Generic arrays, generic bounds in a for-head, and the shift
    // operators themselves -- so the fixture also pins `>>` NOT being a
    // bracket pair.
    void shiftsAreNotBrackets() {
        int a = 1;
        int shifted = a >> 2;
        int unsignedShifted = a >>> 2;
        boolean less = a < 2;
        boolean greater = a > 2;
        a >>= 1;
        a <<= 1;
        a >>>= 1;
        List<List<Integer>> lists = new ArrayList<>();
        for (List<Integer> inner : lists) {
            inner.size();
        }
    }

    // A generic type used as a bound, a nested generic class, and a
    // scoped generic type name.
    static class Node<E> {
        E value;
        Node<E> next;
    }

    Map.Entry<String, List<String>> scopedGeneric;

    Generics<T>.Inner inner;

    class Inner {
    }

    // Casts to a generic type, to a wildcard type, and an intersection
    // cast -- three shapes that all start with `(` and a type.
    @SuppressWarnings("unchecked")
    void casts(Object o) {
        List<String> generic = (List<String>) o;
        List<?> wildcard = (List<?>) o;
        Function<String, String> intersection = (Function<String, String> & Serializable) x -> x;
        generic.size();
        wildcard.size();
        intersection.apply("x");
    }
}
