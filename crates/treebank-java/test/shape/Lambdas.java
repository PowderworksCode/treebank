// Lambdas (8) and method references (8). Both are places where the parser
// cannot tell what it is reading until it reaches the arrow or the `::`:
// `(a)` is a parenthesised expression until `->` arrives, and `A.b` is a
// field access until `::` does.
package fixtures;

import java.util.ArrayList;
import java.util.List;
import java.util.Map;
import java.util.function.BiFunction;
import java.util.function.Function;
import java.util.function.IntBinaryOperator;
import java.util.function.Supplier;

class Lambdas {

    // The three parameter spellings: bare identifier, empty parens,
    // parenthesised list.
    Function<Integer, Integer> bare = x -> x * 2;
    Supplier<String> none = () -> "value";
    IntBinaryOperator two = (a, b) -> a + b;

    // Explicitly typed, `var`-typed, `final`-modified, and annotated
    // parameters -- four ways to write the same list.
    BiFunction<String, String, String> typed = (String a, String b) -> a + b;
    BiFunction<String, String, String> inferred = (var a, var b) -> a + b;
    BiFunction<String, String, String> finalled = (final String a, final String b) -> a + b;
    BiFunction<String, String, String> annotated = (@Deprecated String a, String b) -> a + b;

    // Expression body versus block body, and a block body that yields
    // through `return`.
    Function<Integer, Integer> expression = x -> x + 1;
    Function<Integer, Integer> block = x -> {
        int doubled = x * 2;
        return doubled;
    };

    // A lambda returning a lambda, and one taking a lambda -- currying and
    // higher order, where the arrows nest.
    Function<Integer, Function<Integer, Integer>> curried = a -> b -> a + b;

    <T, R> R apply(Function<T, R> f, T value) {
        return f.apply(value);
    }

    void nested() {
        apply(x -> apply(y -> y, x), "s");
    }

    // A lambda in argument position, in a ternary, in a cast, and in an
    // array initialiser.
    void positions() {
        List<String> items = new ArrayList<>();
        items.forEach(item -> System.out.println(item));
        items.removeIf(item -> item.isEmpty());
        Runnable chosen = items.isEmpty() ? () -> {
        } : () -> items.clear();
        Runnable cast = (Runnable) () -> items.clear();
        Runnable[] array = { () -> items.clear(), () -> items.isEmpty() };
        chosen.run();
        cast.run();
        array[0].run();
    }

    // Method references: static, bound instance, unbound instance,
    // constructor, array constructor, super, and a generic one with
    // explicit type arguments.
    Function<String, Integer> staticRef = Integer::parseInt;
    Supplier<Integer> boundRef = "abc"::length;
    Function<String, Integer> unboundRef = String::length;
    Supplier<List<String>> constructorRef = ArrayList::new;
    Function<Integer, String[]> arrayRef = String[]::new;
    Function<String, List<String>> genericRef = Lambdas::<String>singleton;

    static <T> List<T> singleton(T value) {
        List<T> list = new ArrayList<>();
        list.add(value);
        return list;
    }

    class Inner extends Lambdas {
        Supplier<List<String>> superRef = super::empty;
    }

    List<String> empty() {
        return new ArrayList<>();
    }

    // A qualified method reference through a scoped type name, and one
    // through a generic type.
    Function<String, Integer> qualified = java.lang.Integer::parseInt;
    Function<Map.Entry<String, String>, String> onGeneric = Map.Entry::getKey;
}
