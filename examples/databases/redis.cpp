#include <hiredis/hiredis.h>
#include <cstring>
#include <iostream>

int main() {
    redisReader* reader = redisReaderCreate();
    if (reader == nullptr) return 1;
    const char response[] = "+PONG\r\n";
    if (redisReaderFeed(reader, response, sizeof(response) - 1) != REDIS_OK) {
        redisReaderFree(reader);
        return 2;
    }
    void* parsed = nullptr;
    const int status = redisReaderGetReply(reader, &parsed);
    const auto* reply = static_cast<redisReply*>(parsed);
    const bool valid = status == REDIS_OK && reply != nullptr
        && reply->type == REDIS_REPLY_STATUS && reply->str != nullptr
        && std::strcmp(reply->str, "PONG") == 0;
    if (parsed != nullptr) freeReplyObject(parsed);
    redisReaderFree(reader);
    if (!valid) return 3;
    std::cout << "Redis reply: PONG\n";
}
