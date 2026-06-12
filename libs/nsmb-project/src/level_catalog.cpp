#include "nsmb/project/level_catalog.hpp"

#include <sstream>
#include <string_view>

namespace nsmb::project {

namespace {

constexpr std::string_view nsmbe_level_names = R"(-World 1
1-1|A01_1|A01_2|A01_3
1-2|A02_1|A02_2|A02_3
1-3|A03_1
W1 Tower|A07_1|A07_2|A07_3
1-4|A04_1|A04_2|A04_3
1-5|A05_1|A05_2
W1 Castle|A08_1|A08_2|A08_3
1-A|A06_1|A06_2
Cannon (W1 to W5)|A09_1

-World 2
2-1|B01_1|B01_2|B01_3
2-2|B02_1|B02_2
2-3|B03_1|B03_2
2-4|B04_1|B04_2
W2 Tower|B08_1|B08_2|B08_3
2-5|B05_1|B05_2
2-6|B06_1|B06_2
W2 Castle|B09_1|B09_2|B09_3
2-A|B07_1|B07_2|B07_3
Cannon (W2 to W5)|B10_1

-World 3
3-1|C01_1|C01_2
3-2|C02_1|C02_2
W3 Tower|C08_1|C08_2
3-3|C03_1|C03_2|C03_3
W3 Ghost House|C07_1|C07_2|C07_3
W3 Castle|C09_1|C09_2
3-A|C04_1|C04_2|C04_3
3-B|C05_1|C05_2
3-C|C06_1|C06_2
Cannon (W3 to W6)|C10_1

-World 4 (secret)
4-1|D01_1
4-2|D02_1|D02_2
4-3|D03_1|D03_2
W4 Tower|D09_1|D09_2
4-4|D04_1|D04_2
W4 Ghost House|D08_1|D08_2|D08_3
4-5|D05_1|D05_2|D05_3
4-6|D06_1|D06_2
W4 Castle|D10_1|D10_2
4-A|D07_1|D07_2
Cannon (W4 to W7)|D11_1

-World 5
5-1|E01_1
5-2|E02_1|E02_2
W5 Tower|E09_1|E09_2
5-3|E03_1|E03_2
W5 Ghost House|E08_1|E08_2|E08_3
5-4|E04_1
W5 Castle|E10_1|E10_2
5-A|E05_1|E05_2
5-B|E06_1
5-C|E07_1|E07_2
Cannon (W5 to W8)|E11_1

-World 6
6-1|F01_1|F01_2
6-2|F02_1|F02_2
W6 Tower 1|F09_1|F09_2
6-3|F03_1|F03_2
6-4|F04_1
W6 Tower 2|F10_1|F10_2
6-5|F05_1|F05_2
6-6|F06_1|F06_2|F06_3
W6 Castle|F11_1|F11_2
6-A|F07_1|F07_2
6-B|F08_1
Unused Cannon (W6 to W8)|F12_1

-World 7 (secret)
7-1|G01_1
W7 Ghost House|G09_1|G09_2|G09_3
7-2|G02_1|G02_2
7-3|G03_1|G03_2
W7 Tower|G10_1|G10_2
7-4|G04_1
7-5|G05_1|G05_2|G05_3
7-6|G06_1|G06_2|G06_3
7-7|G07_1
7-A|G08_1|G08_2
W7 Castle|G11_1|G11_2

-World 8
8-1|H01_1|H01_2
8-2|H02_1|H02_2|H02_3
W8 Tower 1|H09_1|H09_2
8-3|H03_1|H03_2
8-4|H04_1|H04_2
W8 Castle|H11_1|H11_2
8-5|H05_1|H05_2
8-6|H06_1|H06_2
8-7|H07_1
8-8|H08_1|H08_2
W8 Tower 2|H10_1|H10_2
W8 Final Castle|H12_1|H12_2|H12_3

-Toad Houses
1-Up Bonus Game|I01_1
Item Bonus Game|I02_1
Mega Mushroom Bonus Game|I03_1
Bottom Screen Background Chooser|I04_1

-Mario vs. Luigi > 2 Cards (Needs the 'Ultimate No Spriteset Limitation' code hack otherwise download play levels will be used!)
SMB 1-1|J01_1
Underground|J02_1
Ice World|J03_1
Pipe World|J04_1
Castle|J05_1

-Mario vs. Luigi > Download Play (Spritesets and background slots can not be changed!)
SMB 1-1|J01_1@Dat_Field
Underground|J02_1@Dat_Basement
Ice World|J03_1@Dat_Ice
Pipe World|J04_1@Dat_Pipe
Castle|J05_1@Dat_Fort

-Unused Levels
Grassland|J06_1
Castle|J07_1
Grassland Déjà vu|J08_1
Desert|J09_1
E3 Desert|J10_1)";

std::string trim(std::string value) {
    const auto first = value.find_first_not_of(" \t\r\n");
    if (first == std::string::npos) {
        return {};
    }
    const auto last = value.find_last_not_of(" \t\r\n");
    return value.substr(first, last - first + 1);
}

std::vector<std::string> split(std::string_view value, char delimiter) {
    std::vector<std::string> parts;
    std::size_t start = 0;
    while (start <= value.size()) {
        const auto end = value.find(delimiter, start);
        if (end == std::string_view::npos) {
            parts.emplace_back(value.substr(start));
            break;
        }
        parts.emplace_back(value.substr(start, end - start));
        start = end + 1U;
    }
    return parts;
}

LevelCatalogArea make_area(const std::string& level_name, std::size_t area_count, std::size_t area_index, std::string token) {
    LevelCatalogArea area;
    area.display_name = area_count == 1U ? level_name : "Area " + std::to_string(area_index);
    area.source_token = std::move(token);

    const auto at = area.source_token.find('@');
    if (at == std::string::npos) {
        area.course_token = area.source_token;
    } else {
        area.course_token = area.source_token.substr(0, at);
        area.narc_name = area.source_token.substr(at + 1U);
    }
    return area;
}

std::vector<LevelCatalogEntry> parse_catalog() {
    std::vector<LevelCatalogEntry> entries;
    std::istringstream input{std::string(nsmbe_level_names)};
    std::string current_world;
    std::string line;

    while (std::getline(input, line)) {
        line = trim(std::move(line));
        if (line.empty()) {
            continue;
        }
        if (line.front() == '-') {
            current_world = line.substr(1);
            continue;
        }

        auto parts = split(line, '|');
        if (parts.size() < 2U || current_world.empty()) {
            continue;
        }

        LevelCatalogEntry entry;
        entry.world = current_world;
        entry.display_name = parts.front();
        const auto area_count = parts.size() - 1U;
        entry.areas.reserve(area_count);
        for (std::size_t i = 1; i < parts.size(); ++i) {
            entry.areas.push_back(make_area(entry.display_name, area_count, i, std::move(parts[i])));
        }
        entries.push_back(std::move(entry));
    }

    return entries;
}

} // namespace

const std::vector<LevelCatalogEntry>& nsmb_level_catalog() {
    static const auto catalog = parse_catalog();
    return catalog;
}

} // namespace nsmb::project
