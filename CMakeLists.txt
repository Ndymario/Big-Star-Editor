cmake_minimum_required(VERSION 3.20)
project(BigStarEditor VERSION 0.4.0 LANGUAGES CXX)

option(BSE_BUILD_QT "Build the Qt 6 desktop frontend when Qt is available" ON)
option(BSE_BUILD_TESTS "Build unit tests" ON)

set(CMAKE_CXX_STANDARD 20)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_CXX_EXTENSIONS OFF)

include(GNUInstallDirs)
include(CTest)

add_subdirectory(libs/nsmb-core)
add_subdirectory(libs/nsmb-nds)
add_subdirectory(libs/nsmb-formats)

if(EMSCRIPTEN)
  # The browser build links only the portable editing slice. Process launch,
  # project-directory I/O, CLI, and Qt stay out of the WebAssembly module.
  add_subdirectory(apps/nsmb-web)
else()
  add_subdirectory(libs/nsmb-project)
  add_subdirectory(libs/nsmb-patching)
  add_subdirectory(apps/nsmb-cli)
  add_subdirectory(apps/nsmb-web)
endif()

if(BSE_BUILD_QT AND NOT EMSCRIPTEN)
  find_package(Qt6 COMPONENTS Widgets QUIET)
  if(Qt6_FOUND)
    add_subdirectory(apps/nsmb-qt)
  else()
    message(STATUS "Qt6 Widgets not found; skipping apps/nsmb-qt. Set BSE_BUILD_QT=OFF to silence this optional target.")
  endif()
endif()
