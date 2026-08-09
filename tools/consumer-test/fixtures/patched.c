#include <stddef.h>

typedef struct node { struct node *next; unsigned flags : 3; } node_t;

__attribute__((nonnull(1))) static size_t count(const node_t *n) {
    size_t k = 0;
    for (const node_t *p = n; p; p = p->next) k++;
    return k;
}

int main(void) { return (int)count(NULL); }
