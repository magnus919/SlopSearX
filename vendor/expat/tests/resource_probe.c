#include <errno.h>
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <unistd.h>
extern int ssx_worker_prepare(void);
static int memory_probe(void) {
#if defined(__linux__)
  if (ssx_worker_prepare())
    return 20;
  struct rlimit r;
  if (getrlimit(RLIMIT_AS, &r) || r.rlim_cur != 1024ULL * 1024 * 1024 ||
      r.rlim_max != r.rlim_cur)
    return 21;
  void *blocks[33] = {0};
  int count = 0;
  for (; count < 33; count++) {
    blocks[count] = malloc(32 * 1024 * 1024);
    if (!blocks[count])
      break;
    volatile unsigned char *p = blocks[count];
    for (size_t i = 0; i < 32 * 1024 * 1024; i += 4096)
      p[i] = 1;
  }
  struct rusage usage;
  if (getrusage(RUSAGE_SELF, &usage))
    return 22;
  for (int i = 0; i < count; i++)
    free(blocks[i]);
  if (count >= 32 || usage.ru_maxrss > 1024 * 1024)
    return 23;
  printf("GNU_AS=1073741824 allocated_blocks=%d peak_rss_kib=%ld\n", count,
         usage.ru_maxrss);
  return 0;
#else
  puts("developer-platform: no absolute RSS qualification");
  return 0;
#endif
}
int main(int argc, char **argv) {
  if (argc == 2 && !strcmp(argv[1], "--memory"))
    return memory_probe();
  if (argc != 3)
    return 2;
  int a = atoi(argv[1]), b = atoi(argv[2]);
  if (fcntl(a, F_GETFD) < 0 || fcntl(b, F_GETFD) < 0)
    return 3;
  if (ssx_worker_prepare())
    return 4;
  errno = 0;
  if (fcntl(a, F_GETFD) >= 0 || errno != EBADF)
    return 5;
  errno = 0;
  if (fcntl(b, F_GETFD) >= 0 || errno != EBADF)
    return 6;
  for (int i = 0; i < 3; i++)
    if (fcntl(i, F_GETFD) < 0)
      return 7;
  errno = 0;
  int n = open("/dev/null", O_RDONLY);
  if (n >= 0 || errno != EMFILE)
    return 8;
  if (write(1, "stdio-ok\n", 9) != 9)
    return 9;
  return 0;
}
