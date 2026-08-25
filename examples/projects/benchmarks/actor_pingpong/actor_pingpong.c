// 基准: actor_pingpong —— 5 万次线程间同步往返（mutex + 双槽 condvar）
// 与 actor_pingpong.rl 逻辑严格一致。输出 = 1250025000
// 对应: Rlyeh actor ask 同步往返（接收方 count+1 回传），C 侧用
//       主线程 <-> worker 线程双槽握手实现同构语义。
#include <pthread.h>
#include <stdio.h>
#include <stdint.h>

#define N 50000

static pthread_mutex_t mtx = PTHREAD_MUTEX_INITIALIZER;
static pthread_cond_t cv_send = PTHREAD_COND_INITIALIZER;
static pthread_cond_t cv_recv = PTHREAD_COND_INITIALIZER;
static int slotA_ready = 0, slotB_ready = 0;
static int64_t slotA = 0, slotB = 0;

static void *worker(void *arg) {
    (void)arg;
    int64_t count = 0;
    for (int64_t i = 0; i < N; i++) {
        pthread_mutex_lock(&mtx);
        while (!slotA_ready) pthread_cond_wait(&cv_send, &mtx);
        slotA_ready = 0;
        (void)slotA;
        count++;
        slotB = count;
        slotB_ready = 1;
        pthread_cond_signal(&cv_recv);
        pthread_mutex_unlock(&mtx);
    }
    return NULL;
}

int main(void) {
    pthread_t t;
    pthread_create(&t, NULL, worker, NULL);
    int64_t sum = 0;
    for (int64_t i = 0; i < N; i++) {
        pthread_mutex_lock(&mtx);
        slotA = i;
        slotA_ready = 1;
        pthread_cond_signal(&cv_send);
        while (!slotB_ready) pthread_cond_wait(&cv_recv, &mtx);
        sum += slotB;
        slotB_ready = 0;
        pthread_mutex_unlock(&mtx);
    }
    pthread_join(t, NULL);
    printf("%lld\n", sum);
    return 0;
}
