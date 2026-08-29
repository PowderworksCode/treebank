// Annotations, in every position the JLS puts them: declarations, types,
// parameters, type parameters and receivers. Where the annotation sits
// decides who owns it, and the type-use family is the one that moves --
// `@Nullable String` annotates the type, `@Deprecated String f` annotates
// the field, and the two look identical until you ask which node they
// hang from.
package fixtures;

import java.lang.annotation.Documented;
import java.lang.annotation.ElementType;
import java.lang.annotation.Repeatable;
import java.lang.annotation.Retention;
import java.lang.annotation.RetentionPolicy;
import java.lang.annotation.Target;
import java.util.List;
import java.util.Map;

@Documented
@Retention(RetentionPolicy.RUNTIME)
@Target({ ElementType.TYPE_USE, ElementType.FIELD, ElementType.PARAMETER })
@interface Nullable {
}

@Retention(RetentionPolicy.RUNTIME)
@Repeatable(Tag.Tags.class)
@interface Tag {
    String value();

    @Retention(RetentionPolicy.RUNTIME)
    @interface Tags {
        Tag[] value();
    }
}

// An annotation type with defaults, an array member, a nested enum constant
// default, and a Class-valued member -- the four member shapes that exist.
@interface Config {
    String name() default "";

    int[] weights() default { 1, 2, 3 };

    Class<?> target() default Object.class;

    RetentionPolicy policy() default RetentionPolicy.SOURCE;
}

@Tag("first")
@Tag("second")
@Config(name = "annotated", weights = { 4, 5 }, target = String.class)
class Annotations {

    // Marker on a field, versus a type-use annotation on the field's type.
    @Deprecated
    private String plain;

    private @Nullable String annotatedType;

    // Type-use annotations inside generics and on array dimensions.
    private List<@Nullable String> elements;

    private Map<String, @Nullable List<@Nullable String>> nested;

    private String @Nullable [] array;

    // A single-element annotation whose value is itself an annotation, and
    // one whose value is an array of them.
    @Config(name = "single")
    private int single;

    // Annotations on a type parameter, a receiver parameter, a formal
    // parameter, and a varargs dimension.
    <@Nullable T> void receiver(@Nullable Annotations this, @Deprecated T value) {
    }

    void varargs(Object @Nullable ... items) {
    }

    // On a constructor, a local, a lambda parameter, and a catch clause.
    @Deprecated
    Annotations() {
        @Nullable String local = null;
        Runnable r = () -> {
        };
        try {
            r.run();
        } catch (@Nullable RuntimeException e) {
            throw e;
        }
    }

    // On an enum constant, and on the enum itself.
    @Deprecated
    enum Kind {
        @Deprecated
        FIRST,
        SECOND(2);

        Kind() {
        }

        Kind(int n) {
        }
    }

    // A qualified annotation name, and a marker with empty parentheses.
    @java.lang.Deprecated()
    void qualified() {
    }
}
