// 基准: actor_pingpong —— 5 万次线程间同步往返（mutex + 双槽 condvar）
// 与 actor_pingpong.rl 逻辑严格一致。输出 = 1250025000
#include <cstdio>
#include <cstdint>
#include <thread>
#include <mutex>
#include <condition_variable>

const int64_t N = 50000;
std::mutex mtx;
std::condition_variable cv_send, cv_recv;
bool slotA_ready = false, slotB_ready = false;
int64_t slotA = 0, slotB = 0;

int main() {
    std::thread worker([&]() {
        int64_t count = 0;
        for (int64_t i = 0; i < N; i++) {
            std::unique_lock<std::mutex> lk(mtx);
            cv_send.wait(lk, [] { return slotA_ready; });
            slotA_ready = false;
            (void)slotA;
            count++;
            slotB = count;
            slotB_ready = true;
            cv_recv.notify_one();
        }
    });
    int64_t sum = 0;
    for (int64_t i = 0; i < N; i++) {
        std::unique_lock<std::mutex> lk(mtx);
        slotA = i;
        slotA_ready = true;
        cv_send.notify_one();
        cv_recv.wait(lk, [] { return slotB_ready; });
        sum += slotB;
        slotB_ready = false;
    }
    worker.join();
    printf("%lld\n", sum);
    return 0;
}
