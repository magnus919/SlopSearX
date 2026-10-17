#define _GNU_SOURCE 1
/* Narrow owned synchronous Expat boundary; no external resources or parser
 * handles exported. */
#include <expat.h>
#include <limits.h>
#include <stddef.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>
typedef int (*Start)(void *, const char *);
typedef int (*End)(void *);
typedef int (*Text)(void *, const char *, size_t);
typedef struct {
  size_t limit, live, peak, retained;
  int denied;
} Budget;
typedef union {
  max_align_t align;
  struct {
    size_t length;
  } value;
} Header;
static _Thread_local Budget *budget;
static void *bounded_malloc(size_t n) {
  if (!budget || n > SIZE_MAX - sizeof(Header) ||
      n + sizeof(Header) > budget->limit - budget->live) {
    if (budget)
      budget->denied = 1;
    return NULL;
  }
  Header *h = malloc(sizeof(Header) + n);
  if (!h)
    return NULL;
  h->value.length = n;
  budget->live += n + sizeof(Header);
  if (budget->live > budget->peak)
    budget->peak = budget->live;
  return h + 1;
}
static void bounded_free(void *p) {
  if (!p)
    return;
  Header *h = (Header *)p - 1;
  budget->live -= h->value.length + sizeof(Header);
  free(h);
}
static void *bounded_realloc(void *p, size_t n) {
  if (!p)
    return bounded_malloc(n);
  if (!n) {
    bounded_free(p);
    return NULL;
  }
  Header *h = (Header *)p - 1;
  void *q = bounded_malloc(n);
  if (!q)
    return NULL;
  memcpy(q, p, n < h->value.length ? n : h->value.length);
  bounded_free(p);
  return q;
}
/* Rust capacities share this same live quota. A retained refusal is pending
 * source projection failure, not a native malloc failure. */
int ssx_retain_charge(size_t n) {
  if (!budget || n > budget->limit - budget->live)
    return 0;
  budget->live += n;
  budget->retained += n;
  if (budget->live > budget->peak)
    budget->peak = budget->live;
  return 1;
}
void ssx_retain_release(size_t n) {
  if (!budget || n > budget->retained)
    abort();
  budget->retained -= n;
  budget->live -= n;
}
typedef struct {
  XML_Parser parser;
  void *user;
  Start start;
  End end;
  Text text;
  int callback_failed, undefined;
} Context;
static void on_start(void *u, const XML_Char *n, const XML_Char **a) {
  (void)a;
  Context *c = u;
  if (!c->callback_failed && !c->undefined && !c->start(c->user, n)) {
    c->callback_failed = 1;
    XML_StopParser(c->parser, XML_FALSE);
  }
}
static void on_end(void *u, const XML_Char *n) {
  (void)n;
  Context *c = u;
  if (!c->callback_failed && !c->undefined && !c->end(c->user)) {
    c->callback_failed = 1;
    XML_StopParser(c->parser, XML_FALSE);
  }
}
static void on_text(void *u, const XML_Char *s, int n) {
  Context *c = u;
  if (!c->callback_failed && !c->undefined &&
      (n < 0 || !c->text(c->user, s, (size_t)n))) {
    c->callback_failed = 1;
    XML_StopParser(c->parser, XML_FALSE);
  }
}
static void on_default(void *u, const XML_Char *s, int n) {
  Context *c = u;
  if (n >= 2 && s[0] == '&') {
    c->undefined = 1;
    XML_StopParser(c->parser, XML_FALSE);
  }
}
/* 0 success,1 source syntax,2 allocation,3 callback failure,4 version/config.
 * peak counts requested native allocation payload+headers, including realloc
 * overlap. It is NOT process RSS; caller's global resource envelope includes
 * other memory. */
int ssx_expat_parse(const char *input, size_t length, size_t memory_limit,
                    void *user, Start start, End end, Text text, size_t *peak) {
  if (budget || strcmp(XML_ExpatVersion(), "expat_2.9.0"))
    return 4;
  int dtd = 0, ge = 0, ns = 0, blap_amp = 0, blap_threshold = 0, alloc_amp = 0,
      alloc_threshold = 0;
  for (const XML_Feature *f = XML_GetFeatureList();
       f->feature != XML_FEATURE_END; f++) {
    if (f->feature == XML_FEATURE_DTD)
      dtd = 1;
    if (f->feature == XML_FEATURE_GE)
      ge = 1;
    if (f->feature == XML_FEATURE_NS)
      ns = 1;
    if (f->feature ==
            XML_FEATURE_BILLION_LAUGHS_ATTACK_PROTECTION_MAXIMUM_AMPLIFICATION_DEFAULT &&
        f->value == 100)
      blap_amp = 1;
    if (f->feature ==
            XML_FEATURE_BILLION_LAUGHS_ATTACK_PROTECTION_ACTIVATION_THRESHOLD_DEFAULT &&
        f->value == 8388608)
      blap_threshold = 1;
    if (f->feature == XML_FEATURE_ALLOC_TRACKER_MAXIMUM_AMPLIFICATION_DEFAULT &&
        f->value == 100)
      alloc_amp = 1;
    if (f->feature == XML_FEATURE_ALLOC_TRACKER_ACTIVATION_THRESHOLD_DEFAULT &&
        f->value == 67108864)
      alloc_threshold = 1;
  }
  if (!dtd || !ge || !ns || !blap_amp || !blap_threshold || !alloc_amp ||
      !alloc_threshold)
    return 4;
  Budget b = {memory_limit, 0, 0, 0, 0};
  budget = &b;
  const XML_Memory_Handling_Suite suite = {bounded_malloc, bounded_realloc,
                                           bounded_free};
  const XML_Char separator = '}';
  XML_Parser p = XML_ParserCreate_MM("utf-8", &suite, &separator);
  if (!p) {
    int failed = b.live ? 4 : 2;
    budget = NULL;
    return failed;
  }
  Context c = {p, user, start, end, text, 0, 0};
  XML_SetUserData(p, &c);
  XML_SetElementHandler(p, on_start, on_end);
  XML_SetCharacterDataHandler(p, on_text);
  XML_SetDefaultHandlerExpand(p, on_default);
  if (!XML_SetParamEntityParsing(p, XML_PARAM_ENTITY_PARSING_NEVER)) {
    XML_ParserFree(p);
    budget = NULL;
    return 4;
  }
  int result = 0;
  size_t offset = 0;
  while (offset < length) {
    size_t n = length - offset;
    if (n > 65536)
      n = 65536;
    if (XML_Parse(p, input + offset, (int)n, XML_FALSE) == XML_STATUS_ERROR) {
      result = 1;
      break;
    }
    offset += n;
  }
  if (!result && XML_Parse(p, "", 0, XML_TRUE) == XML_STATUS_ERROR)
    result = 1;
  if (c.undefined)
    result = 1;
  if (XML_GetErrorCode(p) == XML_ERROR_NO_MEMORY || b.denied)
    result = 2;
  if (c.callback_failed)
    result = 3;
  XML_ParserFree(p);
  *peak = b.peak;
  if (b.live != b.retained)
    result = 4;
  budget = NULL;
  return result;
}

/* Post-exec, before reading untrusted input. No parser or resource loader can
 * inherit descriptors beyond stdio. This is resource containment, not
 * sandboxing. */
#include <dirent.h>
#include <errno.h>
#include <sys/resource.h>
#include <unistd.h>
#if defined(__APPLE__)
#include <mach/mach.h>
#endif
static int set_limit(int resource, rlim_t value) {
  struct rlimit r = {value, value};
  return setrlimit(resource, &r);
}
int ssx_worker_prepare(void) {
  const char *dir =
#if defined(__APPLE__)
      "/dev/fd";
#else
      "/proc/self/fd";
#endif
  DIR *d = opendir(dir);
  if (!d)
    return 1;
  int owned = dirfd(d);
  struct dirent *e;
  for (;;) {
    errno = 0;
    e = readdir(d);
    if (!e) {
      if (errno) {
        closedir(d);
        return 1;
      }
      break;
    }
    char *end;
    errno = 0;
    long n = strtol(e->d_name, &end, 10);
    if (!errno && !*end && n >= 3 && n <= INT_MAX && n != owned) {
      if (close((int)n) && errno != EBADF) {
        closedir(d);
        return 1;
      }
    }
  }
  if (closedir(d))
    return 1;
  if (set_limit(RLIMIT_CORE, 0) || set_limit(RLIMIT_FSIZE, 0) ||
      set_limit(RLIMIT_NOFILE, 3) || set_limit(RLIMIT_CPU, 300))
    return 1;
  rlim_t address = 1024ULL * 1024 * 1024;
#if defined(__APPLE__)
  /* Developer-only headroom; this is NOT an absolute RSS promise. macOS server
   * is outside ADR005's initial release matrix. Production GNU uses fixed AS.
   */
  struct mach_task_basic_info info;
  mach_msg_type_number_t count = MACH_TASK_BASIC_INFO_COUNT;
  if (task_info(mach_task_self(), MACH_TASK_BASIC_INFO, (task_info_t)&info,
                &count) != KERN_SUCCESS)
    return 1;
  if (info.virtual_size > UINT64_MAX - address)
    return 1;
  address += info.virtual_size;
#endif
  if (set_limit(RLIMIT_AS, address))
    return 1;
  return 0;
}

/* Called only by native unit tests, not exposed by worker protocol. */
int ssx_allocator_selftest(void) {
  if (budget)
    return 1;
  Budget b = {4096, 0, 0, 0, 0};
  budget = &b;
  if (bounded_malloc(SIZE_MAX) != NULL || !b.denied || b.live) {
    budget = NULL;
    return 2;
  }
  b.denied = 0;
  unsigned char *p = bounded_malloc(128);
  if (!p) {
    budget = NULL;
    return 3;
  }
  memset(p, 0x5a, 128);
  size_t old_live = b.live;
  b.limit = old_live;
  void *q = bounded_realloc(p, 256);
  if (q || !b.denied || b.live != old_live || p[0] != 0x5a || p[127] != 0x5a) {
    bounded_free(p);
    budget = NULL;
    return 4;
  }
  b.limit = 4096;
  b.denied = 0;
  q = bounded_realloc(p, 256);
  if (!q) {
    bounded_free(p);
    budget = NULL;
    return 5;
  }
  p = q;
  if (p[0] != 0x5a || p[127] != 0x5a || b.live != 256 + sizeof(Header) ||
      b.peak != 128 + 256 + 2 * sizeof(Header)) {
    bounded_free(p);
    budget = NULL;
    return 6;
  }
  if (bounded_realloc(p, 0) != NULL || b.live) {
    budget = NULL;
    return 7;
  }
  p = bounded_realloc(NULL, 0);
  if (!p || b.live != sizeof(Header)) {
    budget = NULL;
    return 8;
  }
  bounded_free(p);
  if (!ssx_retain_charge(2048) || b.live != 2048 || b.retained != 2048) {
    budget = NULL;
    return 9;
  }
  if (bounded_malloc(2048) != NULL || !b.denied) {
    budget = NULL;
    return 10;
  }
  ssx_retain_release(2048);
  p = bounded_malloc(2048);
  if (!p) {
    budget = NULL;
    return 11;
  }
  bounded_free(p);
  int result = b.live || b.retained ? 12 : 0;
  budget = NULL;
  return result;
}
