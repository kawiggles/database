package main

import (
	"bufio"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"net"
	"os"
	"slices"
)

const HEADER_SIZE = 44

func main() {
	addr := "localhost:55555"
	conn, err := net.Dial("tcp", addr)
	if err != nil {
		fmt.Printf("Error connecting to database: %v\n", err)
		os.Exit(1)
	}
	defer conn.Close()

	info := getDbInfo(conn)

	scanner := bufio.NewScanner(os.Stdin)
	for {
		fmt.Print("Database prompt: ")

		if scanner.Scan() {
			input := scanner.Text()
			if input == "EXIT" {
				break;
			}

			call := parseInput(input)

			// This will go inside some call execution function
			_, err := conn.Write(call[0])
			if err != nil {
				fmt.Printf("Error writing to socket: %v\n", err)
			}

			response := make([]byte, 4096)
			n, err := conn.Read(response)
			if err != nil {
				fmt.Printf("Error reading server response: %v\n", err)
			}

			// TODO: function to potentially create new file from response (will depend on some bit)
			fmt.Println(string(response[:n]))
		}
	}

	if err := scanner.Err(); err != nil {
		fmt.Printf("Error closing scanner: %T", err)
	}
}

// Function to convert user input to output
func parseInput(input string) [][]byte {
}

func sendFile(path string, dbInfo DbInfo) ([][]byte, error) {
	info, err := os.Stat(path)
	if err != nil {
		fmt.Printf("Error reading file: %v\n", err)
		return nil, err
	}

	fileSize := info.Size()
	file, err := os.Open(path)
	if err !=nil {
		fmt.Printf("Error reading file: %v\n", err)
		return nil, err
	}

	defer file.Close()

	var messages [][]byte

	if fileSize > int64(dbInfo.ValueSize) {
		var chunks [][]byte

		buf := make([]byte, dbInfo.ValueSize)

		for {
			n, err := file.Read(buf)
			if n > 0 {
				chunks = append(chunks, buf[:n])
			}
			if err == io.EOF {
				break
			}
			if err != nil {
				return nil, err
			}
		}

		for i, chunk := range chunks {
			var message string
			if i == 0 {
				// header message here has to say that this is a multi message file
			}
			// then the rest have a common header message

			messages = slices.Insert(messages, i, []byte(message))
		}
	}

	return messages, nil
}

type DbInfo struct {
	Version uint32 `json:"version"`
	PageSize uint `json:"page_size"`
	ValueSize uint `json:"-"`

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

	err = json.Unmarshal(response, &info)
	if err != nil {
		fmt.Printf("Error when reading metadata response: %v\n", err)
		os.Exit(1)
	}

	info.ValueSize = info.PageSize - HEADER_SIZE

	fmt.Println("Metadata retrieved!")
	return info
}
