interface ServiceIconProps {
  type: 'box' | 'database'
}

function ServiceIcon({ type }: ServiceIconProps) {
  if (type === 'database') {
    return (
      <svg
        xmlns="http://www.w3.org/2000/svg"
        viewBox="0 0 24 24"
        fill="currentColor"
        className="h-4 w-4"
      >
        <path d="M12 2C7.58 2 4 3.79 4 6s3.58 4 8 4 8-1.79 8-4-3.58-4-8-4zM4 9v3c0 2.21 3.58 4 8 4s8-1.79 8-4V9c-.55.42-1.6.8-2.4 1.1C16.32 10.36 14.25 11 12 11s-4.32-.64-5.6-.9C5.6 9.8 4.55 9.42 4 9zm0 6v3c0 2.21 3.58 4 8 4s8-1.79 8-4v-3c-.55.42-1.6.8-2.4 1.1-1.28.26-3.35.9-5.6.9s-4.32-.64-5.6-.9C5.6 15.8 4.55 15.42 4 15z" />
      </svg>
    )
  }

  return (
    <svg
      xmlns="http://www.w3.org/2000/svg"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth={2}
      strokeLinecap="round"
      strokeLinejoin="round"
      className="h-4 w-4"
    >
      <path d="M21 8a2 2 0 0 0-1-1.73l-7-4a2 2 0 0 0-2 0l-7 4A2 2 0 0 0 3 8v8a2 2 0 0 0 1 1.73l7 4a2 2 0 0 0 2 0l7-4A2 2 0 0 0 21 16Z" />
      <path d="m3.3 7 8.7 5 8.7-5" />
      <path d="M12 22V12" />
    </svg>
  )
}

export default ServiceIcon
