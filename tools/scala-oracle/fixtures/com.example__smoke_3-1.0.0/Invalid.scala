// Oracle smoke fixture — the file `oracle.smoke.invalid` points at.
//
// Invalid in EVERY Scala dialect, so it tests that the oracle still
// discriminates rather than that one dialect is fussier than another. It
// shares the Scala 3 coordinate of Valid.scala on purpose: the pair differs
// only in whether the source parses.
object A { def f( = 1 }
