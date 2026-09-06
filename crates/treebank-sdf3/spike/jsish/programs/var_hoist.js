function main() {
  console.log(x);
  var x = 1;
  if (1) {
    var x = 2;
  }
  console.log(x);
  {
    let x = 3;
    console.log(x);
  }
  console.log(x);
}
main();
