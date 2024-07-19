#include <unistd.h>

int main()
{
  execv("./echo", NULL);
  return 0;
}
