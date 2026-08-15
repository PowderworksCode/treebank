// Oracle smoke fixture — the file `oracle.smoke.valid` points at.
//
// It lives under a directory named for a Maven coordinate because that is
// how a Scala file declares its dialect in this repo: the corpus gets one
// from the package it was published under, and a file outside the corpus
// has to declare its own. `lang/scala.rs::dialect_for` refuses to guess, so
// without the `_3` in that directory name there is no verdict to give.
//
// The syntax below is Scala 3 ONLY — scalameta's Scala213 rejects every line
// of it. So this fixture asserts more than "the oracle runs": if dialect
// routing ever broke and sent this file to a Scala 2 dialect, the smoke test
// would see `invalid` and fail, which is the whole point of the language.
package com.example.smoke

enum Colour:
  case Red, Green

given ordering: Ordering[Int] = Ordering.Int

extension (s: String) def twice: String = s * 2

def show(using o: Ordering[Int]): String = o.toString
