package main

import (
	"bufio"
	"fmt"
	"net"
	"os"
	"encoding/binary"
	"bytes"
)

func main() {
	addr := "localhost:55555"
	conn, err := net.Dial("tcp", addr)
	if err != nil {
		fmt.Printf("Error connecting to database: %v\n", err)
		os.Exit(1)
	}
	defer conn.Close()

	scanner := bufio.NewScanner(os.Stdin)
	for {
		fmt.Print("Database prompt: ")

		if scanner.Scan() {
			input := scanner.Text()
			if input == "EXIT" {
				break;
			}

			_, err := conn.Write([]byte(input + "\n"))
			if err != nil {
				fmt.Printf("Error writing to socket: %v\n", err)
			}

			response := make([]byte, 4096)
			n, err := conn.Read(response)
			if err != nil {
				fmt.Printf("Error reading server response: %v\n", err)
			}

			fmt.Println(string(response[:n]))
		}
	}

	if err := scanner.Err(); err != nil {
		fmt.Printf("Error closing scanner: %T", err)
	}
}

func sendFile(path string) [][]byte {
	info, err := os.Stat(path)
	if err != nil {
		fmt.Printf("Error reading file: %v\n", err)
	}

	fileSize := info.Size()
	// Now need to calculate DataPage value size
}

type DbInfo struct {
	Version uint32
	pageSize uint
}

func getDbInfo(conn net.Conn) DbInfo {
	_, err := conn.Write([]byte("INFO\n"))
	if err != nil {
		fmt.Printf("Error retrieving metadata from database: %v\n", err)
		os.Exit(1)
	}
	
	response := make([]byte, 4096)
	_, err = conn.Read(response)
	if err != nil {
		fmt.Printf("Error retrieving metadata from database: %v\n", err)
		os.Exit(1)
	}

	var info DbInfo

	reader := bytes.NewReader(response)
	err = binary.Read(reader, binary.NativeEndian, &info)
	if err != nil {
		fmt.Printf("Error when reading metadata response: %v\n", err)
		os.Exit(1)
	}
	return info
}
