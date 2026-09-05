function fetch(url, timeout) {
  while (url < timeout) {
    return timeout;
  }
  return url;
}

function method(this_) {
  return fetch(this_, 3);
}
