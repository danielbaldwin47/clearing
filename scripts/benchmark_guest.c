/* Independent benchmark harness; runs as init only in an isolated QEMU guest.
 * Full page, dentry and inode caches are dropped before every cold trial.
 * Each warm trial follows an untimed scan by that same executable.
 */
#define _GNU_SOURCE
#include <errno.h>
#include <fcntl.h>
#include <ftw.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mount.h>
#include <sys/reboot.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/wait.h>
#include <time.h>
#include <termios.h>
#include <unistd.h>

#ifndef TRIALS
#define TRIALS 7
#endif
#ifndef GDU_FULL_TREE
#define GDU_FULL_TREE 1
#endif

static unsigned long long audit_file_bytes, audit_directory_bytes;
static unsigned long long audit_files, audit_directories;

static int audit_entry(const char *path, const struct stat *st, int type, struct FTW *ftw) {
    (void)path; (void)ftw;
    if (type == FTW_D) {
        audit_directory_bytes += (unsigned long long)st->st_blocks * 512;
        audit_directories++;
    } else if (type == FTW_F && S_ISREG(st->st_mode)) {
        audit_file_bytes += (unsigned long long)st->st_blocks * 512;
        audit_files++;
    } else {
        return 1;
    }
    return 0;
}

static void die(const char *message) {
    fprintf(stderr, "BENCH_ERROR %s: %s\n", message, strerror(errno));
    fflush(NULL);
    reboot(RB_POWER_OFF);
    _exit(111);
}

static void directory(const char *path) {
    if (mkdir(path, 0755) != 0 && errno != EEXIST) die(path);
}

static long long nanos(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC_RAW, &t)) die("clock_gettime");
    return (long long)t.tv_sec * 1000000000LL + t.tv_nsec;
}

static void drop_caches(void) {
    sync();
    int fd = open("/proc/sys/vm/drop_caches", O_WRONLY);
    if (fd < 0) die("open drop_caches");
    if (write(fd, "3\n", 2) != 2) die("write drop_caches");
    close(fd);
}

static void run(int application, int measured, int trial, const char *cache) {
    const char *name = application ? "clearing" : "gdu";
    int output = open("/tmp/output", O_CREAT | O_TRUNC | O_RDWR, 0600);
    if (output < 0) die("open output");
    long long start = nanos();
    pid_t pid = fork();
    if (pid < 0) die("fork");
    if (!pid) {
        dup2(output, STDOUT_FILENO);
        dup2(output, STDERR_FILENO);
        close(output);
        if (application) {
            execl("/bin/clearing", "clearing", "--scan", "/tree", "--summary", NULL);
        } else {
#if GDU_FULL_TREE
            execl("/bin/gdu", "gdu", "--config-file", "/dev/null", "-n", "-p", "-c", "--no-prefix", "-s",
                  "-o", "-", "--output-attrs", "asize,dsize,items", "/tree", NULL);
#else
            execl("/bin/gdu", "gdu", "--config-file", "/dev/null", "-n", "-p", "-c", "--no-prefix", "-s", "/tree", NULL);
#endif
        }
        _exit(127);
    }
    int status;
    if (waitpid(pid, &status, 0) < 0) die("waitpid");
    long long elapsed = nanos() - start;
    if (!WIFEXITED(status) || WEXITSTATUS(status)) {
        char buffer[4096];
        lseek(output, 0, SEEK_SET);
        ssize_t n;
        while ((n = read(output, buffer, sizeof(buffer))) > 0) write(2, buffer, n);
        die("scanner process failed");
    }
    if (measured) {
        printf("BENCH_RESULT %s %s %d %lld\n", name, cache, trial, elapsed);
        printf("BENCH_OUTPUT_BEGIN\n");
        fflush(stdout);
        lseek(output, 0, SEEK_SET);
        char buffer[4096]; ssize_t n;
        while ((n = read(output, buffer, sizeof(buffer))) > 0) write(1, buffer, n);
        printf("\nBENCH_OUTPUT_END\n");
        fflush(stdout);
    }
    close(output);
}

int main(void) {
    if (getpid() != 1) {
        fputs("This harness runs only as PID 1 in the dedicated benchmark VM.\n", stderr);
        return 2;
    }
    directory("/proc"); directory("/sys"); directory("/dev");
    directory("/tmp"); directory("/tree");
    if (mount("proc", "/proc", "proc", 0, NULL)) die("mount proc");
    if (mount("sysfs", "/sys", "sysfs", 0, NULL)) die("mount sysfs");
    if (mount("devtmpfs", "/dev", "devtmpfs", 0, NULL)) die("mount devtmpfs");
    if (mount("tmpfs", "/tmp", "tmpfs", 0, "size=64m")) die("mount tmpfs");
    if (mount("/dev/vda", "/tree", "ext4", MS_RDONLY | MS_NOATIME, NULL)) die("mount benchmark image");
    setenv("HOME", "/tmp", 1); setenv("LANG", "C", 1); setenv("TERM", "dumb", 1);
    printf("BENCH_ENV cpus=%ld trials=%d cache_reset=sync+drop_caches:3\n", sysconf(_SC_NPROCESSORS_ONLN), TRIALS);
    if (nftw("/tree", audit_entry, 128, FTW_PHYS | FTW_MOUNT)) die("independent fixture audit");
    printf("BENCH_AUDIT {\"file_bytes\":%llu,\"directory_bytes\":%llu,\"files\":%llu,\"directories\":%llu}\n",
           audit_file_bytes, audit_directory_bytes, audit_files, audit_directories);
    fflush(stdout);
    // Untimed startup calls fault scanner libraries in once. Dropping caches
    // later affects both binaries identically; both live in the initramfs.
    run(0, 0, 0, "startup"); run(1, 0, 0, "startup");
    for (int trial = 0; trial < TRIALS; ++trial) {
        for (int position = 0; position < 2; ++position) {
            int application = (trial + position) % 2;
            drop_caches();
            run(application, 1, trial, "cold");
        }
        for (int position = 0; position < 2; ++position) {
            int application = (trial + position + 1) % 2;
            run(application, 0, trial, "warmup");
            run(application, 1, trial, "warm");
        }
    }
    puts("BENCH_COMPLETE"); fflush(NULL);
    tcdrain(STDOUT_FILENO);
    usleep(100000);
    reboot(RB_POWER_OFF);
    return 0;
}
