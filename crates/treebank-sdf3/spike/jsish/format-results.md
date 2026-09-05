# Format results for jsish

8 of 8 programs round-trip, print idempotently, and print exactly what prettier prints.

## PASS: jsish/programs/function_hoist.js

```js
function outer(a) {
  console.log(inner(1));
  function inner(b) {
    return a + b;
  }
  return inner(10);
}
console.log(outer(5));
```

## PASS: jsish/programs/let_block.js

```js
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
```

## PASS: jsish/programs/tdz.js

```js
let y = 1;
{
  let y = y + 1;
  console.log(y);
}
```

## PASS: jsish/programs/var_hoist.js

```js
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
```

## PASS: jsish/programs/var_param.js

```js
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
```

## PASS: rosetta/branching/program.js

```js
function classify(n) {
  if (n < 0) {
    return 1;
  } else {
    return 2;
  }
}
```

## PASS: rosetta/comments/program.js

```js
// a leading comment
function greet(name) {
  let prefix = name; // a trailing comment
  return prefix + name;
}
```

## PASS: rosetta/hello-roles/program.js

```js
function fetch(url, timeout) {
  while (url < timeout) {
    return timeout;
  }
  return url;
}

function method(this_) {
  return fetch(this_, 3);
}
```

