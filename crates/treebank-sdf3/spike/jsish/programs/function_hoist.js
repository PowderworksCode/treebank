function outer(a) {
  console.log(inner(1));
  function inner(b) {
    return a + b;
  }
  return inner(10);
}
console.log(outer(5));
