// The `(` of the first lambda has no `)` in its item, because the `{` in the parentheses counts as a
// bracket. A record of locals that reads the parameters of this lambda reads past the item.
int y; auto f = [=]({) { return y; }; auto g = [&]() { return y; };
