class Fixture {
  void varargs(String @Nullable ... args) {}   // 0002 annotation before ellipsis

  void contextual(SimpleWhen when) {           // 0003 `when` is contextual
    for (SimpleWhen w = when; w != null; w = w.next) {}
  }
}
