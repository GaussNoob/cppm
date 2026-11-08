#include <mongoc/mongoc.h>
#include <iostream>

int main() {
    mongoc_init();
    mongoc_client_t* client = mongoc_client_new("mongodb://127.0.0.1:27017");
    if (client == nullptr) {
        mongoc_cleanup();
        return 1;
    }
    bson_t document;
    bson_init(&document);
    const bool valid = BSON_APPEND_UTF8(&document, "name", "cppm")
        && BSON_APPEND_INT32(&document, "answer", 42)
        && bson_validate(&document, BSON_VALIDATE_NONE, nullptr);
    bson_destroy(&document);
    std::cout << "MongoDB client: " << mongoc_get_version() << '\n';
    mongoc_client_destroy(client);
    mongoc_cleanup();
    return valid ? 0 : 2;
}
