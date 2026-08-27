set(BSE_WEB_BACKEND_SOURCES
  src/c_api.cpp
  ${PROJECT_SOURCE_DIR}/libs/nsmb-project/src/editable_level.cpp
  ${PROJECT_SOURCE_DIR}/libs/nsmb-project/src/module_data.cpp
)

function(configure_web_backend target)
  target_include_directories(${target} PUBLIC include ${PROJECT_SOURCE_DIR}/libs/nsmb-project/include)
  target_link_libraries(${target} PUBLIC nsmb-core nsmb-formats)
  target_compile_features(${target} PRIVATE cxx_std_20)
endfunction()

if(EMSCRIPTEN)
  add_executable(nsmb-web ${BSE_WEB_BACKEND_SOURCES})
  configure_web_backend(nsmb-web)
  set_target_properties(nsmb-web PROPERTIES OUTPUT_NAME big-star SUFFIX ".js")
  target_link_options(nsmb-web PRIVATE
    --no-entry
    -sALLOW_MEMORY_GROWTH=1
    -sENVIRONMENT=web
    -sFILESYSTEM=0
    -sMODULARIZE=1
    -sEXPORT_NAME=BigStarModule
    "-sEXPORTED_FUNCTIONS=['_malloc','_free']"
    -sEXPORTED_RUNTIME_METHODS=HEAPU8
  )
  add_custom_command(TARGET nsmb-web POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E copy_if_different ${CMAKE_CURRENT_SOURCE_DIR}/index.html $<TARGET_FILE_DIR:nsmb-web>/index.html
    COMMAND ${CMAKE_COMMAND} -E copy_if_different ${CMAKE_CURRENT_SOURCE_DIR}/app.js $<TARGET_FILE_DIR:nsmb-web>/app.js
    COMMAND ${CMAKE_COMMAND} -E copy_if_different ${CMAKE_CURRENT_SOURCE_DIR}/style.css $<TARGET_FILE_DIR:nsmb-web>/style.css
  )
else()
  add_library(nsmb-web-api STATIC ${BSE_WEB_BACKEND_SOURCES})
  configure_web_backend(nsmb-web-api)
  add_custom_target(nsmb-web-assets SOURCES index.html app.js style.css README.md)
  if(BSE_BUILD_TESTS)
    add_executable(nsmb-web-api-tests tests/c_api_tests.cpp)
    target_link_libraries(nsmb-web-api-tests PRIVATE nsmb-web-api)
    add_test(NAME nsmb-web-api-tests COMMAND nsmb-web-api-tests)
  endif()
endif()
