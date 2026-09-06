function f(a) {
  var a = a + 1;
  return a;
}
function g(a) {
  a = a + 1;
  var b = 40;
  {
    var b = b + 2;
  }
  return a + b;
}
console.log(f(1));
console.log(g(1));
