add_library(nsmb-formats
  src/g2d.cpp
  src/graphics.cpp
  src/glue.cpp
  src/identify.cpp
  src/level.cpp
  src/message.cpp
  src/narc.cpp
  src/tex0.cpp
  src/tileset.cpp
  src/ui_resources.cpp
)
add_library(nsmb::formats ALIAS nsmb-formats)
target_include_directories(nsmb-formats PUBLIC include)
target_link_libraries(nsmb-formats PUBLIC nsmb-core nsmb-nds)
target_compile_features(nsmb-formats PUBLIC cxx_std_20)

if(BSE_BUILD_TESTS)
  add_executable(nsmb-formats-tests tests/graphics_tests.cpp)
  target_link_libraries(nsmb-formats-tests PRIVATE nsmb-formats)
  add_test(NAME nsmb-formats-tests COMMAND nsmb-formats-tests)
  add_executable(nsmb-glue-tests tests/glue_tests.cpp)
  target_link_libraries(nsmb-glue-tests PRIVATE nsmb-formats)
  add_test(NAME nsmb-glue-tests COMMAND nsmb-glue-tests)
  add_executable(nsmb-g2d-tests tests/g2d_tests.cpp)
  target_link_libraries(nsmb-g2d-tests PRIVATE nsmb-formats)
  add_test(NAME nsmb-g2d-tests COMMAND nsmb-g2d-tests)
  add_executable(nsmb-tex0-tests tests/tex0_tests.cpp)
  target_link_libraries(nsmb-tex0-tests PRIVATE nsmb-formats)
  add_test(NAME nsmb-tex0-tests COMMAND nsmb-tex0-tests)
  add_executable(nsmb-tileset-tests tests/tileset_tests.cpp)
  target_link_libraries(nsmb-tileset-tests PRIVATE nsmb-formats)
  add_test(NAME nsmb-tileset-tests COMMAND nsmb-tileset-tests)
  add_executable(nsmb-message-tests tests/message_tests.cpp)
  target_link_libraries(nsmb-message-tests PRIVATE nsmb-formats)
  add_test(NAME nsmb-message-tests COMMAND nsmb-message-tests)
  add_executable(nsmb-ui-resources-tests tests/ui_resources_tests.cpp)
  target_link_libraries(nsmb-ui-resources-tests PRIVATE nsmb-formats)
  add_test(NAME nsmb-ui-resources-tests COMMAND nsmb-ui-resources-tests)
endif()
