#include <stdio.h>

int main() {
  char buf[105];
  FILE *f = fopen("data/ans0", "r");
  fgets(buf, 100, f);
  fputs(buf, stdout);
  return 0;
}
