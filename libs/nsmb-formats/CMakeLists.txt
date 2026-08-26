add_library(nsmb-formats
  src/graphics.cpp
  src/identify.cpp
  src/level.cpp
)
add_library(nsmb::formats ALIAS nsmb-formats)
target_include_directories(nsmb-formats PUBLIC include)
target_link_libraries(nsmb-formats PUBLIC nsmb-core)
target_compile_features(nsmb-formats PUBLIC cxx_std_20)

if(BSE_BUILD_TESTS)
  add_executable(nsmb-formats-tests tests/graphics_tests.cpp)
  target_link_libraries(nsmb-formats-tests PRIVATE nsmb-formats)
  add_test(NAME nsmb-formats-tests COMMAND nsmb-formats-tests)
endif()
