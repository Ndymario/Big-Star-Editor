add_library(nsmb-project
  src/editable_level.cpp
  src/effective_filesystem.cpp
  src/filesystem_view.cpp
  src/graphics_resources.cpp
  src/level_catalog.cpp
  src/level_exchange.cpp
  src/lua_actors.cpp
  src/lua_sandbox.cpp
  src/lua_tiles.cpp
  src/manifest.cpp
  src/module_data.cpp
  src/patch_apply.cpp
  src/project.cpp
  src/rom_reader.cpp
  src/sprite_definitions.cpp
)
find_package(Lua 5.4 REQUIRED)
add_library(nsmb::project ALIAS nsmb-project)
target_include_directories(nsmb-project PUBLIC include)
target_include_directories(nsmb-project PRIVATE ${LUA_INCLUDE_DIR})
target_link_libraries(nsmb-project PUBLIC nsmb-core nsmb-nds nsmb-formats nsmb-patching PRIVATE ${LUA_LIBRARIES})
target_compile_features(nsmb-project PUBLIC cxx_std_20)
if(BSE_BUILD_TESTS)
  add_executable(nsmb-project-tests tests/project_tests.cpp)
  target_link_libraries(nsmb-project-tests PRIVATE nsmb-project)
  add_test(NAME nsmb-project-tests COMMAND nsmb-project-tests)
  add_executable(nsmb-module-data-tests tests/module_data_tests.cpp)
  target_link_libraries(nsmb-module-data-tests PRIVATE nsmb-project)
  add_test(NAME nsmb-module-data-tests COMMAND nsmb-module-data-tests)
  add_executable(nsmb-sprite-definitions-tests tests/sprite_definitions_tests.cpp)
  target_link_libraries(nsmb-sprite-definitions-tests PRIVATE nsmb-project)
  add_test(NAME nsmb-sprite-definitions-tests COMMAND nsmb-sprite-definitions-tests)
  add_executable(nsmb-lua-actors-tests tests/lua_actors_tests.cpp)
  target_link_libraries(nsmb-lua-actors-tests PRIVATE nsmb-project)
  target_compile_definitions(nsmb-lua-actors-tests PRIVATE BSE_TEST_SOURCE_DIR="${PROJECT_SOURCE_DIR}")
  add_test(NAME nsmb-lua-actors-tests COMMAND nsmb-lua-actors-tests)
  add_executable(nsmb-lua-tiles-tests tests/lua_tiles_tests.cpp)
  target_link_libraries(nsmb-lua-tiles-tests PRIVATE nsmb-project)
  target_compile_definitions(nsmb-lua-tiles-tests PRIVATE BSE_TEST_SOURCE_DIR="${PROJECT_SOURCE_DIR}")
  add_test(NAME nsmb-lua-tiles-tests COMMAND nsmb-lua-tiles-tests)
  add_executable(nsmb-level-exchange-tests tests/level_exchange_tests.cpp)
  target_link_libraries(nsmb-level-exchange-tests PRIVATE nsmb-project)
  add_test(NAME nsmb-level-exchange-tests COMMAND nsmb-level-exchange-tests)
  add_executable(nsmb-rom-reader-tests tests/rom_reader_tests.cpp)
  target_link_libraries(nsmb-rom-reader-tests PRIVATE nsmb-project)
  add_test(NAME nsmb-rom-reader-tests COMMAND nsmb-rom-reader-tests)
  add_executable(nsmb-effective-filesystem-tests tests/effective_filesystem_tests.cpp)
  target_link_libraries(nsmb-effective-filesystem-tests PRIVATE nsmb-project)
  add_test(NAME nsmb-effective-filesystem-tests COMMAND nsmb-effective-filesystem-tests)
  add_executable(nsmb-filesystem-view-tests tests/filesystem_view_tests.cpp)
  target_link_libraries(nsmb-filesystem-view-tests PRIVATE nsmb-project)
  add_test(NAME nsmb-filesystem-view-tests COMMAND nsmb-filesystem-view-tests)
endif()
