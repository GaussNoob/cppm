#include <mysql.h>
#include <iostream>

int main() {
    if (mysql_library_init(0, nullptr, nullptr) != 0) return 1;
    MYSQL* client = mysql_init(nullptr);
    if (client == nullptr) {
        mysql_library_end();
        return 2;
    }
    std::cout << "MySQL/MariaDB client: " << mysql_get_client_info() << '\n';
    mysql_close(client);
    mysql_library_end();
}
