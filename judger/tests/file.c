#include <stdio.h>

int main() {
  FILE *f = fopen("test_file", "w");
  fprintf(f, "test\n");
}
