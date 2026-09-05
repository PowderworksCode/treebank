let y = 1;
{
  let y = 2;
  {
    console.log(y);
    let y = 3;
    console.log(y);
  }
  console.log(y);
}
console.log(y);
