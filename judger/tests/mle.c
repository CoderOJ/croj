#include <stdlib.h>
#include <string.h>

int main()
{
  for (;;)
  {
    void *p = malloc(1024);
    memset(p, 0, 1024);
  }
}
