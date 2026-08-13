// Invalid in EVERY Scala dialect (verified against scalameta's Scala211,
// Scala212, Scala213 and Scala3), so this tests the grammar's strictness
// rather than the dialect union that patched.scala pins.
object A { def f( = 1 }
