add_executable(nsmb-cli main.cpp)
target_link_libraries(nsmb-cli PRIVATE nsmb-project nsmb-patching)
target_compile_definitions(nsmb-cli PRIVATE BSE_VERSION="${PROJECT_VERSION}")
target_compile_features(nsmb-cli PRIVATE cxx_std_20)
