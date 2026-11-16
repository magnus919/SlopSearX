/* Native qualification launcher only; never linked into the backend. */
#define _POSIX_C_SOURCE 200809L
#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

static int kill_and_reap(pid_t child, int *status) {
  if (kill(child, SIGKILL) && errno != ESRCH)
    return 0;
  pid_t reaped;
  do {
    reaped = waitpid(child, status, 0);
  } while (reaped < 0 && errno == EINTR);
  return reaped == child;
}

static long long monotonic_ms(void) {
  struct timespec now;
  if (clock_gettime(CLOCK_MONOTONIC, &now))
    return -1;
  return (long long)now.tv_sec * 1000 + now.tv_nsec / 1000000;
}

int main(int argc, char **argv) {
  if (argc != 3 && argc != 4 && argc != 5)
    return 40;
  unsigned long timeout = 10000;
  if (argc >= 4) {
    char *end;
    errno = 0;
    timeout = strtoul(argv[3], &end, 10);
    if (errno || *end || timeout == 0 || timeout > 10000)
      return 40;
  }
  int await_started = argc == 5;
  if (await_started && strcmp(argv[4], "--await-started"))
    return 40;
  int sentinel = open(argv[2], O_CREAT | O_EXCL | O_WRONLY, 0600);
  int sockets[2];
  if (sentinel < 0)
    return 41;
  if (socketpair(AF_UNIX, SOCK_STREAM, 0, sockets)) {
    close(sentinel);
    return 41;
  }
  pid_t child = fork();
  if (child < 0) {
    close(sentinel);
    close(sockets[0]);
    close(sockets[1]);
    return 42;
  }
  if (child == 0) {
    char file_fd[32], socket_fd[32];
    close(sockets[1]);
    alarm(8);
    if (snprintf(file_fd, sizeof(file_fd), "%d", sentinel) < 0 ||
        snprintf(socket_fd, sizeof(socket_fd), "%d", sockets[0]) < 0)
      _exit(43);
    execl(argv[1], argv[1], file_fd, socket_fd, (char *)NULL);
    _exit(44);
  }
  close(sentinel);
  close(sockets[0]);
  int status = 0;
  long long started = monotonic_ms();
  long long startup_started = started;
  long long startup_elapsed = 0;
  int acknowledged = !await_started;
  for (;;) {
    pid_t reaped = waitpid(child, &status, WNOHANG);
    if (reaped == child)
      break;
    if (reaped < 0 && errno != EINTR) {
      /* ECHILD does not authorize signaling a possibly reused process ID. */
      if (errno == ECHILD) {
        close(sockets[1]);
        return 49;
      }
      int cleaned = kill_and_reap(child, &status);
      close(sockets[1]);
      return cleaned ? 48 : 49;
    }
    long long now = monotonic_ms();
    if (await_started && !acknowledged && now >= 0 &&
        now - startup_started < 5000) {
      struct stat marker;
      if (stat(argv[2], &marker) == 0 && marker.st_size > 0) {
        acknowledged = 1;
        startup_elapsed = now - startup_started;
        started = now;
      }
    }
    unsigned long phase_timeout = acknowledged ? timeout : 5000;
    if (started < 0 || now < 0 || now - started >= (long long)phase_timeout) {
      int cleaned = kill_and_reap(child, &status);
      close(sockets[1]);
      if (!cleaned)
        return 49;
      if (started < 0 || now < 0 || !WIFSIGNALED(status) ||
          WTERMSIG(status) != SIGKILL)
        return 48;
      if (await_started) {
        printf("{\"owned_child\":%ld,\"reaped\":true,\"signal\":%d,"
               "\"reason\":\"%s\",\"startup_acknowledged\":%s,"
               "\"startup_elapsed_ms\":%lld}\n",
               (long)child, WTERMSIG(status),
               acknowledged ? "deadline" : "startup_deadline",
               acknowledged ? "true" : "false", startup_elapsed);
      } else {
        printf("{\"owned_child\":%ld,\"reaped\":true,\"signal\":%d,\"reason\":\"deadline\"}\n",
               (long)child, WTERMSIG(status));
      }
      return 47;
    }
    struct timespec pause_time = {.tv_sec = 0, .tv_nsec = 1000000};
    if (nanosleep(&pause_time, NULL) && errno != EINTR) {
      int cleaned = kill_and_reap(child, &status);
      close(sockets[1]);
      return cleaned ? 48 : 49;
    }
  }
  struct pollfd peer = {.fd = sockets[1], .events = POLLIN};
  int ready = poll(&peer, 1, 1000);
  char byte;
  int eof = ready > 0 && read(sockets[1], &byte, 1) == 0;
  close(sockets[1]);
  if (!eof || !WIFEXITED(status) || WEXITSTATUS(status))
    return 46;
  return 0;
}
