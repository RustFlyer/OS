# Install script for directory: /home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/fs_test

# Set the install prefix
if(NOT DEFINED CMAKE_INSTALL_PREFIX)
  set(CMAKE_INSTALL_PREFIX "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install")
endif()
string(REGEX REPLACE "/$" "" CMAKE_INSTALL_PREFIX "${CMAKE_INSTALL_PREFIX}")

# Set the install configuration name.
if(NOT DEFINED CMAKE_INSTALL_CONFIG_NAME)
  if(BUILD_TYPE)
    string(REGEX REPLACE "^[^A-Za-z0-9_]+" ""
           CMAKE_INSTALL_CONFIG_NAME "${BUILD_TYPE}")
  else()
    set(CMAKE_INSTALL_CONFIG_NAME "Release")
  endif()
  message(STATUS "Install configuration: \"${CMAKE_INSTALL_CONFIG_NAME}\"")
endif()

# Set the component getting installed.
if(NOT CMAKE_INSTALL_COMPONENT)
  if(COMPONENT)
    message(STATUS "Install component: \"${COMPONENT}\"")
    set(CMAKE_INSTALL_COMPONENT "${COMPONENT}")
  else()
    set(CMAKE_INSTALL_COMPONENT)
  endif()
endif()

# Install shared libraries without execute permission?
if(NOT DEFINED CMAKE_INSTALL_SO_NO_EXE)
  set(CMAKE_INSTALL_SO_NO_EXE "1")
endif()

# Is this installation the result of a crosscompile?
if(NOT DEFINED CMAKE_CROSSCOMPILING)
  set(CMAKE_CROSSCOMPILING "TRUE")
endif()

# Set path to fallback-tool for dependency-resolution.
if(NOT DEFINED CMAKE_OBJDUMP)
  set(CMAKE_OBJDUMP "/opt/riscv/riscv64-linux-musl-cross/bin/riscv64-linux-musl-objdump")
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(EXISTS "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-server" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-server")
    file(RPATH_CHECK
         FILE "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-server"
         RPATH "")
  endif()
  list(APPEND CMAKE_ABSOLUTE_DESTINATION_FILES
   "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-server")
  if(CMAKE_WARN_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(WARNING "ABSOLUTE path INSTALL DESTINATION : ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  if(CMAKE_ERROR_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(FATAL_ERROR "ABSOLUTE path INSTALL DESTINATION forbidden (by caller): ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  file(INSTALL DESTINATION "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin" TYPE EXECUTABLE FILES "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/fs_test/lwext4-server")
  if(EXISTS "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-server" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-server")
    if(CMAKE_INSTALL_DO_STRIP)
      execute_process(COMMAND "/opt/riscv/riscv64-linux-musl-cross/bin/riscv64-linux-musl-strip" "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-server")
    endif()
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  include("/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/fs_test/CMakeFiles/lwext4-server.dir/install-cxx-module-bmi-Release.cmake" OPTIONAL)
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(EXISTS "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-client" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-client")
    file(RPATH_CHECK
         FILE "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-client"
         RPATH "")
  endif()
  list(APPEND CMAKE_ABSOLUTE_DESTINATION_FILES
   "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-client")
  if(CMAKE_WARN_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(WARNING "ABSOLUTE path INSTALL DESTINATION : ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  if(CMAKE_ERROR_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(FATAL_ERROR "ABSOLUTE path INSTALL DESTINATION forbidden (by caller): ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  file(INSTALL DESTINATION "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin" TYPE EXECUTABLE FILES "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/fs_test/lwext4-client")
  if(EXISTS "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-client" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-client")
    if(CMAKE_INSTALL_DO_STRIP)
      execute_process(COMMAND "/opt/riscv/riscv64-linux-musl-cross/bin/riscv64-linux-musl-strip" "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-client")
    endif()
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  include("/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/fs_test/CMakeFiles/lwext4-client.dir/install-cxx-module-bmi-Release.cmake" OPTIONAL)
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(EXISTS "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-generic" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-generic")
    file(RPATH_CHECK
         FILE "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-generic"
         RPATH "")
  endif()
  list(APPEND CMAKE_ABSOLUTE_DESTINATION_FILES
   "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-generic")
  if(CMAKE_WARN_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(WARNING "ABSOLUTE path INSTALL DESTINATION : ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  if(CMAKE_ERROR_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(FATAL_ERROR "ABSOLUTE path INSTALL DESTINATION forbidden (by caller): ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  file(INSTALL DESTINATION "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin" TYPE EXECUTABLE FILES "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/fs_test/lwext4-generic")
  if(EXISTS "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-generic" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-generic")
    if(CMAKE_INSTALL_DO_STRIP)
      execute_process(COMMAND "/opt/riscv/riscv64-linux-musl-cross/bin/riscv64-linux-musl-strip" "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-generic")
    endif()
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  include("/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/fs_test/CMakeFiles/lwext4-generic.dir/install-cxx-module-bmi-Release.cmake" OPTIONAL)
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(EXISTS "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mkfs" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mkfs")
    file(RPATH_CHECK
         FILE "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mkfs"
         RPATH "")
  endif()
  list(APPEND CMAKE_ABSOLUTE_DESTINATION_FILES
   "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mkfs")
  if(CMAKE_WARN_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(WARNING "ABSOLUTE path INSTALL DESTINATION : ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  if(CMAKE_ERROR_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(FATAL_ERROR "ABSOLUTE path INSTALL DESTINATION forbidden (by caller): ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  file(INSTALL DESTINATION "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin" TYPE EXECUTABLE FILES "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/fs_test/lwext4-mkfs")
  if(EXISTS "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mkfs" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mkfs")
    if(CMAKE_INSTALL_DO_STRIP)
      execute_process(COMMAND "/opt/riscv/riscv64-linux-musl-cross/bin/riscv64-linux-musl-strip" "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mkfs")
    endif()
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  include("/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/fs_test/CMakeFiles/lwext4-mkfs.dir/install-cxx-module-bmi-Release.cmake" OPTIONAL)
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  if(EXISTS "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mbr" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mbr")
    file(RPATH_CHECK
         FILE "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mbr"
         RPATH "")
  endif()
  list(APPEND CMAKE_ABSOLUTE_DESTINATION_FILES
   "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mbr")
  if(CMAKE_WARN_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(WARNING "ABSOLUTE path INSTALL DESTINATION : ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  if(CMAKE_ERROR_ON_ABSOLUTE_INSTALL_DESTINATION)
    message(FATAL_ERROR "ABSOLUTE path INSTALL DESTINATION forbidden (by caller): ${CMAKE_ABSOLUTE_DESTINATION_FILES}")
  endif()
  file(INSTALL DESTINATION "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin" TYPE EXECUTABLE FILES "/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/fs_test/lwext4-mbr")
  if(EXISTS "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mbr" AND
     NOT IS_SYMLINK "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mbr")
    if(CMAKE_INSTALL_DO_STRIP)
      execute_process(COMMAND "/opt/riscv/riscv64-linux-musl-cross/bin/riscv64-linux-musl-strip" "$ENV{DESTDIR}/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/install/bin/lwext4-mbr")
    endif()
  endif()
endif()

if(CMAKE_INSTALL_COMPONENT STREQUAL "Unspecified" OR NOT CMAKE_INSTALL_COMPONENT)
  include("/home/unix/file/OSkernel/myOS/rfos/lib/lwext4_rust/c/lwext4/build_musl-generic/fs_test/CMakeFiles/lwext4-mbr.dir/install-cxx-module-bmi-Release.cmake" OPTIONAL)
endif()

