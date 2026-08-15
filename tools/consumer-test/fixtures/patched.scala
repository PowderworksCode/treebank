// One construct per treebank-scala grammar patch, plus the dialect claim.
//
// Patch 0003 — `enum` as a term name. Scala 2 code uses `enum` as an
// ordinary identifier; Scala 3 made it a keyword. Upstream accepted it only
// inside import paths, so Flink's `def qualifyEnum(enum: Enum[_])` and its
// uses failed to parse. Six corpus files, all Apache Flink.
class EnumValueSerializer[E](val enum: E)

def qualifyEnum(enum: Enum[_]): String =
  enum.getClass.getCanonicalName + "." + enum.name()

// The rest pins the DIALECT the ledger claims. tree-sitter-scala is ONE
// grammar for TWO languages, so the grammar is the union of Scala 2 and
// Scala 3 while the oracle is per file — which is why ledger.json has to
// record how each file's dialect was decided. Deliberately, NO single
// scalameta dialect accepts this whole file: the `enum` term name above is
// rejected by Scala3, and everything below is rejected by Scala213.
enum Colour:
  case Red, Green

given intOrdering: Ordering[Int] = Ordering.Int

extension (s: String) def twice: String = s * 2

def show(using o: Ordering[Int]): String = o.toString
