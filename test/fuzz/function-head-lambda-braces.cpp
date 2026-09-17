// The head of a function whose default argument is a lambda. The `(` of the lambda has no `)` in the
// item, because the two `{` count as brackets. A record of locals that reads its parameters reads past
// the item.
int k;
void f(Callback cb = [](int a {{)) {
  return;
}
