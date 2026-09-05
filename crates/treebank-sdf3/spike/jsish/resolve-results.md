# Resolution results for jsish

5 of 5 programs print, under resolution from bindings.json alone, what node prints.

## PASS: function_hoist.js

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

| | output |
|---|---|
| bindings.json | `6 15` |
| node | `6 15` |

## PASS: let_block.js

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

| | output |
|---|---|
| bindings.json | `error: y used before its binding` |
| node | `error: ReferenceError: Cannot access 'y' before initialization` |

## PASS: tdz.js

```js
let y = 1;
{
  let y = y + 1;
  console.log(y);
}
```

| | output |
|---|---|
| bindings.json | `error: y used before its binding` |
| node | `error: ReferenceError: Cannot access 'y' before initialization` |

## PASS: var_hoist.js

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

| | output |
|---|---|
| bindings.json | `undefined 2 3 2` |
| node | `undefined 2 3 2` |

## PASS: var_param.js

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

| | output |
|---|---|
| bindings.json | `2 44` |
| node | `2 44` |

