// Identifiers are not ASCII. JLS 3.8 defines them by
// `Character.isJavaIdentifierStart`/`Part`, and BOTH are predicates over
// Unicode general categories rather than character lists:
//
//   start  L, Nl, Sc, Pc
//   part   the above plus Nd, Mn, Mc, Cf
//
// Every category appears below, because the grammar's identifier rule was
// written from what ASCII code looks like and had only L, Nd, `_` and `$`
// until issue #196. The combining marks were the visible half of that: a
// name like `चूंकि` ended at the first matra and 53 corpus files
// did not parse. Nl, Sc, Pc and Cf were missing for the same reason and
// are pinned here, since no corpus file spells them.
//
// The other side of the boundary -- a mark, a digit or a format character
// trying to START a name -- is in test/negative/, where a fixture that
// must be REJECTED belongs.
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

    // ── the categories the rule used to omit ───────────────────────

    // Mn (non-spacing) and Mc (combining spacing) marks, which is what
    // issue #196 was. Devanagari, Tamil, Kannada, Malayalam, Telugu,
    // Gujarati, Thai and an Arabic diacritic -- one per script that the
    // cucumber corpus files failed on.
    int चूंकि = 20;
    int ஆனால் = 21;
    int ನೀಡಿದ = 22;
    int എപ്പോൾ = 23;
    int మరియు = 24;
    int આપેલછે = 25;
    int ดังนั้น = 26;
    int اذاً = 27;

    // Nl (letter number), Sc (currency), Pc (connector) -- all three are
    // identifier START characters, which is why `_` and `$` never needed
    // naming separately.
    int Ⅷ = 28;
    int €uro = 29;
    int ‿tie = 30;

    // Cf (format): a zero-width joiner INSIDE a name. Part-only, and
    // `Character.isIdentifierIgnorable` is why javac takes it.
    int zero‍width = 31;

    // The exact shape issue #196 reported: a scoped annotation argument
    // whose type name carries a combining mark. Nothing about annotations
    // was ever wrong -- the lexer simply ended the name early, and this is
    // the line that proved it.
    @संकेत(संकेत.संकेतs.class)
    static class उपयोग {
    }

    @interface संकेत {
        Class<?> value();

        @interface संकेतs {
            संकेत[] value();
        }
    }

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
