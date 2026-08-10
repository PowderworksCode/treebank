struct s { int a; };
int g(int);
int f(void) { return g(struct s); }
