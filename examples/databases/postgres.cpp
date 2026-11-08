#include <libpq-fe.h>
#include <iostream>

int main() {
    if (PQlibVersion() <= 0) return 1;
    PQconninfoOption* options = PQconndefaults();
    if (options == nullptr) return 2;
    std::cout << "PostgreSQL client: " << PQlibVersion() << '\n';
    PQconninfoFree(options);
}
