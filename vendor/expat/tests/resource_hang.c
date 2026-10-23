/* Qualification-only child: acknowledge the inherited sentinel then wait. */
#define _POSIX_C_SOURCE 200809L
#include <stdio.h>
#include <stdlib.h>
#include <unistd.h>
int main(int argc, char **argv) {
  if (argc != 3)
    return 60;
  char *end;
  long fd = strtol(argv[1], &end, 10);
  if (*end || fd < 0)
    return 61;
  char text[64];
  int size = snprintf(text, sizeof(text), "%ld\n", (long)getpid());
  if (size <= 0 || (size_t)size >= sizeof(text) ||
      write((int)fd, text, (size_t)size) != size)
    return 62;
  for (;;)
    pause();
}
