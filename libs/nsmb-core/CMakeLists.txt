add_library(nsmb-core
  src/byte_buffer.cpp
  src/command.cpp
  src/endian.cpp
)
add_library(nsmb::core ALIAS nsmb-core)
target_include_directories(nsmb-core PUBLIC include)
target_compile_features(nsmb-core PUBLIC cxx_std_20)

if(BSE_BUILD_TESTS)
  add_executable(nsmb-core-tests tests/core_tests.cpp)
  target_link_libraries(nsmb-core-tests PRIVATE nsmb-core)
  add_test(NAME nsmb-core-tests COMMAND nsmb-core-tests)
endif()
