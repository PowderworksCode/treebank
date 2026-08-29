// Identifiers are not ASCII. JLS 3.8 defines them by
// `Character.isJavaIdentifierStart`/`Part`, so a letter from any script
// starts one and digits, `_` and `$` continue one.
//
// What is deliberately NOT here: identifiers containing a combining mark
// (Unicode `Mn`/`Mc`), such as Devanagari `चूंकि` or Tamil `ஆனால்`. Those
// are issue #196 -- the grammar's identifier class omits both categories,
// so those files do not parse cleanly yet and a fixture for them belongs
// with the fix, not before it. Every script below is mark-free on purpose.
package fixtures;

class UnicodeIdentifiers {

    // Cyrillic, Greek, Armenian, Georgian, Hebrew, Arabic, Han, Kana,
    // Hangul, Amharic -- all `Lu`/`Ll`/`Lo`, all accepted today.
    int Тогда = 1;
    int Δεδομένου = 2;
    int Բայց = 3;
    int მაშინ = 4;
    int כאשר = 5;
    int عندما = 6;
    int 前提 = 7;
    int かつ = 8;
    int 그리고 = 9;
    int ግን = 10;

    // Latin letters outside ASCII, including the ones with a distinct
    // uppercase form.
    int Þurh = 11;
    int Étantdonné = 12;
    int Zakładając = 13;

    // Digits, `_` and `$` are identifier-PART only, so they may follow a
    // letter but a digit may not lead.
    int Тогда2 = 14;
    int かつ_9 = 15;
    int 前提$x = 16;

    // `_` and `$` may also lead, being connector punctuation and a
    // currency symbol respectively.
    int _leading = 17;
    int $leading = 18;

    // A non-ASCII type name, used as a type, a constructor and a
    // qualified static reference -- the three positions where the same
    // spelling reaches the parser through different rules.
    static class Ταξινόμηση {
        static final int ΣΤΑΘΕΡΑ = 19;

        int πεδίο;

        Ταξινόμηση(int πεδίο) {
            this.πεδίο = πεδίο;
        }

        int μέθοδος() {
            return πεδίο + ΣΤΑΘΕΡΑ;
        }
    }

    Ταξινόμηση δημιουργία() {
        Ταξινόμηση τ = new Ταξινόμηση(Ταξινόμηση.ΣΤΑΘΕΡΑ);
        return τ;
    }

    // A non-ASCII scoped name in an annotation argument, which is the
    // shape #196 reports -- here with a mark-free identifier, so it is
    // the positive control for that issue.
    @SuppressWarnings("unused")
    int πηγή = fixtures.UnicodeIdentifiers.Ταξινόμηση.ΣΤΑΘΕΡΑ;

    // Non-ASCII labels, type parameters and lambda parameters.
    <Τ> Τ ταυτότητα(Τ τιμή) {
        return τιμή;
    }

    void βρόχος() {
        έξω: for (int ι = 0; ι < 3; ι++) {
            if (ι == 1) {
                continue έξω;
            }
            break έξω;
        }
    }
}
