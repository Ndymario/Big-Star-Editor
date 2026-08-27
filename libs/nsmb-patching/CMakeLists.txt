add_library(nsmb-patching
  src/ncpatcher.cpp
  src/process.cpp
  src/legacy.cpp
)
add_library(nsmb::patching ALIAS nsmb-patching)
target_include_directories(nsmb-patching PUBLIC include)
target_link_libraries(nsmb-patching PUBLIC nsmb-core)
find_package(Threads REQUIRED)
target_link_libraries(nsmb-patching PUBLIC Threads::Threads)
target_compile_features(nsmb-patching PUBLIC cxx_std_20)
if(BSE_BUILD_TESTS)
  add_executable(nsmb-patching-tests tests/patching_tests.cpp)
  target_link_libraries(nsmb-patching-tests PRIVATE nsmb-patching)
  add_test(NAME nsmb-patching-tests COMMAND nsmb-patching-tests)
endif()
